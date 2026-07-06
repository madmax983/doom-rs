🧨 **The Trigger:** An out of bounds index into blockmap logic using `.unwrap_or(0)` as fallback for missing offsets yields 0 and parses the blockmap map properties header as map data.
📉 **The Stack Trace:** (OOB Read / Infinite Loop / Logic Bug)
🧪 **Reproduction:** Call `blockmap.block_linedefs(1, 1)` with a `1x1` size map.
😈 **Comment:** You assumed the buffer would always have valid offsets or 0 meant empty. You were wrong.
