🔭 Vantage: Spec for Turn-Based Mode

👤 **User Story:** "As a Player, I want to play the game at my own pace where time only advances when I take an action, so that I can enjoy a tactical, roguelike experience using the new ASCII rendering mode."

✅ **Acceptance Criteria:**
- Time strictly advances based on player action cost, allowing monsters and projectiles to move proportionally.
- The game world correctly updates between player turns without skipping critical simulation steps.
- Players can clearly choose between real-time or turn-based play at startup.

🚫 **Out of Scope:**
- New AI Behaviors
- Grid-Snapping Movement
- Turn-Based Multiplayer
