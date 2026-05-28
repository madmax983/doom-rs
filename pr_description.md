🖌️ **Before:** Fatal engine errors (e.g. failing to load an IWAD file) were printed directly using standard `eprintln!` calls. This output appeared as a raw text block, lacking clear visual hierarchy or separation from other terminal output.

✨ **After:** Engine failures are now intercepted and rendered inside a `comfy-table` with explicit borders, bold red typography, and properly aligned inner reasons (the underlying cause chain). This creates a polished dashboard-like visual that unmistakably separates fatal crashes from normal logs.

🖼️ **Visuals:** An error like a missing IWAD now prints inside a dedicated terminal table titled "❌ Engine Failure" (styled bold red), with a secondary row dedicated to listing the specific OS errors or file problems, fully adopting the "Z-Pattern" layout for quick error scanning.
