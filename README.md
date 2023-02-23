## osm-ways-slope

### Description

Given an osm.pbf file and some elevation data, extract slope information for each way (given an optional filter).

This information could be used to calculate a penalty / modify the expected travel time for each section of a route (see for example http://www.liedman.net/2015/04/13/add-elevation-data-to-osrm/).

### Installation

```bash
git clone https://github.com/mthh/osm-ways-slope
cd osm-ways-slope
cargo build --release
```

### Usage

#### Without filter

```bash
./target/release/osm-ways-slope /path/to/osm/file.osm.pbf /path/to/elevation/file.tif output.json
```

#### With filter

Here is an example of a filter that will only compute slope information for ways that have a highway tag with a value of primary or secondary, or that have a cycleway tag (with any value).

```bash
./target/release/osm-ways-slope /path/to/osm/file.osm.pbf /path/to/elevation/file.tif output.json --filter highway=primary,highway=secondary,cycleway
```
