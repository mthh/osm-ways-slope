use std::collections::BTreeMap;
use clap::Parser;
use serde::{Deserialize, Serialize};
use gdal::raster::{RasterBand, ResampleAlg};
use gdal::{Dataset, Metadata, GeoTransformEx};
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
}

#[derive(Serialize, Deserialize)]
struct WayInfo {
    way_id: i64,
    distance: f64,
    climb_distance: f64,
    descent_distance: f64,
    climb: f64,
    descent: f64,
}

struct Location {
    latitude: f64,
    longitude: f64,
}

fn haversine_distance(start: Location, end: Location) -> f64 {
    let mut r: f64 = 6371.0;

    let d_lat: f64 = (end.latitude - start.latitude).to_radians();
    let d_lon: f64 = (end.longitude - start.longitude).to_radians();
    let lat1: f64 = (start.latitude).to_radians();
    let lat2: f64 = (end.latitude).to_radians();

    let a: f64 = ((d_lat/2.0).sin()) * ((d_lat/2.0).sin()) + ((d_lon/2.0).sin()) * ((d_lon/2.0).sin()) * (lat1.cos()) * (lat2.cos());
    let c: f64 = 2.0 * ((a.sqrt()).atan2((1.0-a).sqrt()));

    return r * c;
}

fn main() {
    let args = Args::parse();

    // Open OSM file
    let r = std::fs::File::open(&Path::new(&args.osm_file)).expect(format!("Unable to open OSM file {}", &args.osm_file).as_str());
    let mut pbf = osmpbfreader::OsmPbfReader::new(r);

    // Open elevation file
    let dataset = Dataset::open(&args.elevation_file).expect(format!("Unable to open elevation file {}", &args.elevation_file).as_str());
    let rasterband: RasterBand = dataset.rasterband(1).unwrap();
    let transform = dataset.geo_transform().unwrap();
    let invert_transform = transform.invert().unwrap();

    // Get all the highways and their dependencies
    let objs = pbf.get_objs_and_deps(|obj| {
        obj.is_way() && obj.tags().contains_key("highway")
    }).unwrap();

    let mut node_elevation = BTreeMap::new();
    let mut result: Vec<WayInfo> = Vec::new();

    // Iterate over all the nodes and get their elevations
    objs.iter()
        .filter(|(id, obj)| {
            if let osmpbfreader::OsmObj::Node(node) = obj {
                true
            } else {
                false
            }
        })
        .for_each(|(id, obj)| {
            let node = obj.node().unwrap();
            let (x, y) = invert_transform.apply(node.lon(), node.lat());
            let elevation = rasterband.read_as::<f64>((x as isize, y as isize), (1, 1), (1, 1), Some(ResampleAlg::Bilinear)).unwrap();
            node_elevation.insert(id.inner_id(), elevation.data[0]);
        });

    // Iterate over all the ways
    objs.iter()
        .filter(|(id, obj)| {
            if let osmpbfreader::OsmObj::Way(way) = obj {
                true
            } else {
                false
            }
        })
        .for_each(|(id, obj)| {
            let way = obj.way().unwrap();
            let mut distance: f64 = 0.0;
            let mut climb_distance: f64 = 0.0;
            let mut descent_distance: f64 = 0.0;
            let mut climb: f64 = 0.0;
            let mut descent: f64 = 0.0;

            way.nodes.iter()
                .zip(way.nodes.iter().skip(1))
                .for_each(|(a, b)| {
                    let node_a = objs.get(&osmpbfreader::OsmId::Node(*a)).unwrap().node().unwrap();
                    let node_b = objs.get(&osmpbfreader::OsmId::Node(*b)).unwrap().node().unwrap();
                    let id_a = a.0;
                    let id_b = b.0;
                    let start = Location {
                        latitude: node_a.lat(),
                        longitude: node_a.lon(),
                    };
                    let end = Location {
                        latitude: node_b.lat(),
                        longitude: node_b.lon(),
                    };
                    distance += haversine_distance(start, end) / 1000.;

                    if node_elevation.get(&id_a).unwrap() < node_elevation.get(&id_b).unwrap() {
                        climb_distance += distance;
                        climb += node_elevation.get(&id_b).unwrap() - node_elevation.get(&id_a).unwrap();
                    } else {
                        descent_distance += distance;
                        descent += node_elevation.get(&id_a).unwrap() - node_elevation.get(&id_b).unwrap();
                    }

                });

            result.push(WayInfo {
                way_id: id.inner_id(),
                distance,
                climb_distance,
                descent_distance,
                climb,
                descent,
            })
        });

    // Serialize result to a JSON string and write it to a file
    let json_str = serde_json::to_string(&result).expect("Unable to serialize result to string");
    std::fs::write(args.output_file, json_str).expect("Unable to write file");
}