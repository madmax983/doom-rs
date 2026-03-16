# Menu & HUD WAD Patch Parity

**Date**: 2026-03-16
**Goal**: Replace text/rectangle placeholder rendering with actual WAD patch graphics for menus, status bar, mugshot face, and HUD font.

## Build Order

### Step 1: Foundation — `draw_patch`, `PatchCache`, `WadFont`

**`Framebuffer::draw_patch(x: i32, y: i32, patch: &PatchImage)`**
- Iterates columns, skips transparent gaps between posts
- Clips to 320x200 screen bounds
- Signed coordinates (patches use left_offset/top_offset)

**`PatchCache`** (`doom-renderer`):
- `HashMap<LumpName, PatchImage>` with lazy loading from WadStack
- `get(&mut self, name: &str, wad: &WadStack) -> Option<&PatchImage>`
- `preload_menu_patches()`, `preload_statusbar_patches()`

**`WadFont`** (`doom-renderer`):
- Loads STCFN033–STCFN095 (ASCII 33–95) as PatchImage glyphs
- Space (ASCII 32) = 4px advance, no patch
- `draw_string(fb, x, y, text)` — variable-width patch-based text
- `string_width(text) -> i32` for centering

### Step 2: Face Animation FSM

**`FaceState`** (`doom-game/src/face.rs`):
- `health_tier: u8` (0–4), `current_face: FaceKind`, `display_tics: u8`, `priority: u8`
- `last_attack_angle: Bam` for damage direction

**`FaceKind` enum**:
- Normal(tier, direction), Pain(tier), Ouch(tier), EvilGrin, Rampage(tier), GodMode, Dead, XDead

**Priority cascade** (per tic, top-down):
1. Dead/XDead (health <= 0)
2. GodMode (invulnerable)
3. Ouch (damage >= 20 this tic, hold 35 tics)
4. Pain (any damage, hold 35 tics)
5. EvilGrin (new weapon pickup, hold 35 tics)
6. Rampage (firing 2+ tics, hold 2 tics)
7. Normal + idle look (random direction ~every 105 tics)

**`face_patch_name(kind) -> String`**: maps to STFST00, STFTL20, STFOUCH3, STFGOD0, etc.

### Step 3: Status Bar Rewrite

Replace text/rectangles with WAD patches drawn at Doom's exact pixel positions.

**Patches**: STBAR, STTNUM0–9, STTMINUS, STTPRCNT, STYSNUM0–9, STARMS, STGNUM2–7, STKEYS0–5, STFB0/STFB1

**`draw_statusbar(fb, patch_cache, statusbar_data, face_state)`**:
1. STBAR background at (0, 168)
2. Ammo at x=44 — STTNUM right-aligned 3 digits
3. Health at x=90 + STTPRCNT
4. Arms box — STARMS bg + STGNUM for owned weapons
5. Face — resolved from FaceState via face_patch_name
6. Armor at x=221 + STTPRCNT
7. Keys at x=239 — STKEYS patches stacked
8. Ammo tally at x=288/314 — STYSNUM digits

**Helper**: `draw_stnum(fb, x, y, value, max_digits, digit_patches)` — right-aligned digit rendering.

### Step 4: Menu Renderer Rewrite

Replace text rendering with WAD menu patches at original positions.

**Per-page patches**:
- Main: M_DOOM + M_NGAME/M_OPTION/M_LOADG/M_SAVEG/M_QUITG
- Episode: M_EPISOD + M_EPI1–4
- Skill: M_NEWG + M_JKILL/M_ROUGH/M_HURT/M_ULTRA/M_NMARE
- Load/Save: M_LOADG/M_SAVEG + 6 slot rectangles with WadFont names
- Options: M_OPTTTL + item patches + thermometer stubs (non-functional)

**Skull cursor**: M_SKULL1/M_SKULL2 at (item_x - 32, item_y), alternating every 8 tics.

**Title/Credits**: TITLEPIC, CREDIT, HELP1/HELP2 — full-screen patches.

### Step 5: Tests

- `draw_patch` clipping (offscreen, partial, zero-size)
- FaceState priority cascade (god > ouch > pain > grin > rampage > idle)
- `face_patch_name` correctness
- `draw_stnum` digit rendering
- WadFont string width calculation
- Integration: `--capture` headless frame export

## Files Changed

| File | Action |
|------|--------|
| `crates/doom-game/src/face.rs` | **New** — FaceState FSM |
| `crates/doom-game/src/lib.rs` | Expose face module |
| `crates/doom-game/src/player.rs` | Add FaceState to PlayerState |
| `crates/doom-renderer/src/framebuffer.rs` | Add `draw_patch()` |
| `crates/doom-renderer/src/lib.rs` | Add PatchCache |
| `crates/doom-renderer/src/wad_font.rs` | **New** — WadFont |
| `crates/doom-renderer/src/statusbar.rs` | Rewrite with patches |
| `crates/doom-renderer/src/menu_render.rs` | Rewrite with patches |

## Not In Scope

- Options menu slider behavior (visual stubs only)
- Finale text crawl
- Pause screen (trivial once draw_patch exists)
