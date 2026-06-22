💡 **The Spark:** We have telemetry tracking player movement, and we have the MapAnalyzer that finds pathfinding data. Can I export the chokepoints and isolated areas to GeoJSON so that mappers can visually analyze tactical regions in standard GIS tools alongside player heatmaps?

🚀 **The Feature:** Implemented `export_analysis_to_geojson` in the `MapAnalyzer`.

🔭 **The Potential:** This bridges the gap between raw map connectivity data and external visualization, letting level designers easily view chokepoints as map markers alongside the structural layout using any standard GeoJSON viewer (like Mapbox or geojson.io).

⚠️ **Risk:** Low. The feature is purely an exporter added to the map analysis module and wired through the CLI behind a new `--export-analysis` flag, completely isolated from runtime execution.
