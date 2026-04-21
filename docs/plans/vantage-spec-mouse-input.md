# 🔭 Vantage: Spec for Mouse Input Support

## 👤 User Story
As a Player, I want to be able to use my mouse to aim and interact with the game, so that I can enjoy a modern, responsive, and intuitive control scheme instead of relying solely on keyboard input.

## ❓ So What?
Currently, the game only supports keyboard input. While this is faithful to the very earliest days of Doom, modern players universally expect and rely on mouse-look (and mouse movement/firing) as the baseline standard for first-person shooters. Without mouse support, the game's control scheme feels archaic and alienates the vast majority of potential users. Implementing mouse support bridges the gap between classic engine mechanics and modern usability expectations, significantly expanding the accessible audience and improving the core gameplay feel.

## 📏 Metric Definition
- **Success Criteria:**
  - The game captures mouse movement events and translates them correctly into player turning and forward/backward movement (if enabled).
  - The game captures mouse button clicks (Left, Right, Middle) and maps them to standard actions like firing or interacting.
  - Mouse sensitivity is configurable and feels natural compared to standard source ports.
  - The mouse cursor can be optionally locked/hidden to prevent it from leaving the application window bounds during play.

## 🔍 Gap Analysis
- **Current State:** The input system entirely lacks definitions and handling for mouse events or mouse-driven state changes. The engine relies solely on keyboard polling.
- **Standard Libs / Market:** Virtually every modern source port (GZDoom, Chocolate Doom, Crispy Doom) provides robust mouse support by default. In standard terminal environments, mouse events are expected features that can be captured and routed to the input loop, providing parity with modern player expectations.

## ✅ Acceptance Criteria
- Must introduce mouse event handling into the core input event loop.
- Must map horizontal mouse movement to player rotation/turning speed in the input handling system.
- Must map mouse button clicks to standard game actions (Left Click = Fire).
- Must provide a toggle to capture the mouse cursor to the application window.
- Must handle the continuous flow of mouse events without dropping inputs or causing input lag.

## 🚫 Out of Scope
- Vertical mouse look (mlook / looking up and down). The core game renderer is 2.5D and does not currently support vertical looking. Mouse input will be strictly horizontal (turning) and buttons (firing).
- Complex custom mouse bindings via a UI menu (Phase 2).
