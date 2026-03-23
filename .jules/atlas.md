## 2024-05-24 - Extract cohesive sub-states from GameState God Struct
**Tangle:** `GameState` was a sprawling God Struct with 27 fields managing diverse domains like player state, level statistics, environmental movers (doors, lifts, ceilings, scrolling walls), and sound propagation all tightly tangled together.
**Blueprint:** Created cohesive sub-structs (`LevelStats`, `SectorMovers`, and `SoundPropagation`) to group related fields, dramatically improving the domain boundary enforcement and reducing struct bloat.
