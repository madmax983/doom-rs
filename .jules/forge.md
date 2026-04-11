>>
**Refactored boolean blindness in activate_crusher and render_automap**
**Learning:** Having many consecutive boolean parameters (like `silent`, `remove_when_done`, `show_all_lines`, `show_all_things`) makes function signatures hard to read and easy to mistakenly swap arguments.
**Action:** Group these configuration parameters into dedicated structs like `CrusherParams` and `AutomapState` to clarify intent.
