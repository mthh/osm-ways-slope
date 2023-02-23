## osm-ways-slope

### Description

...

### Installation

```bash
cargo build --release
```

### Usage

#### Without filter
```bash
osm-ways-slope /path/to/osm/file.osm.pbf /path/to/elevation/file.tif output.json
```

#### With filter

Here is an example of a filter that will only keep ways that have a highway tag with a value of primary, secondary or a cycleway tag (with any value).

```bash
osm-ways-slope /path/to/osm/file.osm.pbf /path/to/elevation/file.tif output.json --filter highway=primary,highway=secondary,cycleway
```
