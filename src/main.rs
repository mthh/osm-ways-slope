use clap::Parser;
use fxhash::FxHashMap;
use gdal::raster::{RasterBand, ResampleAlg};
use gdal::{Dataset, GeoTransformEx};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // The path to the OSM file to process
    osm_file: String,
    // The path to the elevation file to process
    elevation_file: String,
    // The path to the output file
    output_file: String,
    // The key or key-value pair to filter the ways by
    #[arg(short, long)]
    filter: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WayInfo {
    distance: f64,
    climb_distance: f64,
    descent_distance: f64,
    climb: f64,
    descent: f64,
}

#[derive(Debug)]
struct Location {
    latitude: f64,
    longitude: f64,
}

#[derive(Debug)]
enum Filter {
    Key(String),
    KeyValue(String, String),
}

fn haversine_distance(start: Location, end: Location) -> f64 {
    let d_lat: f64 = (end.latitude - start.latitude).to_radians();
    let d_lon: f64 = (end.longitude - start.longitude).to_radians();
    let lat1: f64 = (start.latitude).to_radians();
    let lat2: f64 = (end.latitude).to_radians();

    let a: f64 = ((d_lat / 2.0).sin()) * ((d_lat / 2.0).sin())
        + ((d_lon / 2.0).sin()) * ((d_lon / 2.0).sin()) * (lat1.cos()) * (lat2.cos());
    let c: f64 = 2.0 * ((a.sqrt()).atan2((1.0 - a).sqrt()));

    return 6371.0 * c;
}

// Macro that takes an array of filter and returns a closure that can be used to filter the ways
macro_rules! filter {
    ($filters:expr) => {
        |obj: &osmpbfreader::OsmObj| {
            let mut ret_val = false;
            for filter in $filters {
                match filter {
                    Filter::Key(key) => {
                        if obj.tags().contains_key(key.as_str()) {
                            ret_val = true;
                        }
                    }
                    Filter::KeyValue(key, value) => {
                        if obj.tags().get(key.as_str())
                            == Some(&smartstring::alias::String::from(value.as_str()))
                        {
                            ret_val = true;
                        }
                    }
                }
            }
            ret_val
        }
    };
}

fn main() {
    let args = Args::parse();

    // Read optional arguments if any in order to build an array of filters
    let filters = match args.filter {
        Some(filter) => {
            let split = filter.split(',');
            let mut result = Vec::new();
            for k_or_kv in split {
                let mut split = k_or_kv.split('=');
                let key = split.next().unwrap();
                let value = split.next();
                let ret_val = match value {
                    Some(value) => Filter::KeyValue(key.to_string(), value.to_string()),
                    None => Filter::Key(key.to_string()),
                };
                result.push(ret_val);
            }
            result
        }
        None => vec![Filter::Key("highway".to_string())],
    };

    // Open OSM file
    let r = std::fs::File::open(&Path::new(&args.osm_file))
        .expect(format!("Unable to open OSM file {}", &args.osm_file).as_str());
    let mut pbf = osmpbfreader::OsmPbfReader::new(r);

    // Open elevation file
    let dataset = Dataset::open(&args.elevation_file)
        .expect(format!("Unable to open elevation file {}", &args.elevation_file).as_str());
    let rasterband: RasterBand = dataset.rasterband(1).unwrap();
    let transform = dataset.geo_transform().unwrap();
    let invert_transform = transform.invert().unwrap();

    // Get all the ways, according to the user-defined filter, and their dependencies
    let objs = pbf
        .get_objs_and_deps(|obj| obj.is_way() && filter!(&filters)(obj))
        .unwrap();

    let mut node_elevation: FxHashMap<i64, f64> = FxHashMap::default();

    // Iterate over all the dependant nodes and get their elevations
    objs.iter()
        .filter(|(_id, obj)| {
            if let osmpbfreader::OsmObj::Node(_node) = obj {
                true
            } else {
                false
            }
        })
        .for_each(|(id, obj)| {
            let node = obj.node().unwrap();
            let (x, y) = invert_transform.apply(node.lon(), node.lat());
            let elevation = rasterband
                .read_as::<f64>(
                    (x as isize, y as isize),
                    (1, 1),
                    (1, 1),
                    Some(ResampleAlg::NearestNeighbour),
                )
                .unwrap();
            node_elevation.insert(id.inner_id(), elevation.data[0]);
        });

    // Create an iterator over all our ways
    let filtered_objs = objs.iter().filter(|(_id, obj)| {
        if let osmpbfreader::OsmObj::Way(_way) = obj {
            true
        } else {
            false
        }
    });

    // Resulting map of way_id -> WayInfo
    let mut result_map: FxHashMap<i64, WayInfo> = FxHashMap::with_capacity_and_hasher(
        filtered_objs.size_hint().1.unwrap(),
        Default::default(),
    );

    // Compute slope information for all our ways
    filtered_objs.for_each(|(id, obj)| {
        let way = obj.way().unwrap();
        let mut distance: f64 = 0.0;
        let mut climb_distance: f64 = 0.0;
        let mut descent_distance: f64 = 0.0;
        let mut climb: f64 = 0.0;
        let mut descent: f64 = 0.0;

        way.nodes
            .iter()
            .zip(way.nodes.iter().skip(1))
            .for_each(|(a, b)| {
                let node_a = objs
                    .get(&osmpbfreader::OsmId::Node(*a))
                    .unwrap()
                    .node()
                    .unwrap();
                let node_b = objs
                    .get(&osmpbfreader::OsmId::Node(*b))
                    .unwrap()
                    .node()
                    .unwrap();
                let id_a = &a.0;
                let id_b = &b.0;

                distance += haversine_distance(
                    Location {
                        latitude: node_a.lat(),
                        longitude: node_a.lon(),
                    },
                    Location {
                        latitude: node_b.lat(),
                        longitude: node_b.lon(),
                    },
                ) * 1000.;

                if node_elevation.get(id_a).unwrap() < node_elevation.get(id_b).unwrap() {
                    climb_distance += distance;
                    climb += node_elevation.get(id_b).unwrap() - node_elevation.get(id_a).unwrap();
                } else {
                    descent_distance += distance;
                    descent +=
                        node_elevation.get(id_a).unwrap() - node_elevation.get(id_b).unwrap();
                }
            });

        result_map.insert(
            id.inner_id(),
            WayInfo {
                distance,
                climb_distance,
                descent_distance,
                climb,
                descent,
            },
        );
    });

    // Serialize result to a JSON string and write it to a file
    let json_str =
        serde_json::to_string(&result_map).expect("Unable to serialize result to string");
    std::fs::write(args.output_file, json_str).expect("Unable to write file");
}
