with open('crates/doom-net/src/input_log.rs', 'r') as f:
    data = f.read()
data = data.replace("use doom_net::{InputLog, TicCmd, packet::MAX_PLAYERS};", "use doom_net::{InputLog, packet::MAX_PLAYERS};\n/// use doom_types::TicCmd;")
with open('crates/doom-net/src/input_log.rs', 'w') as f:
    f.write(data)

with open('crates/doom-net/src/packet.rs', 'r') as f:
    data = f.read()
data = data.replace("use doom_net::{TicPacket, TicCmd, packet::MAX_PLAYERS};", "use doom_net::{TicPacket, packet::MAX_PLAYERS};\n/// use doom_types::TicCmd;")
with open('crates/doom-net/src/packet.rs', 'w') as f:
    f.write(data)

with open('crates/doom-net/src/rollback.rs', 'r') as f:
    data = f.read()
data = data.replace("use doom_net::{TicCmd, TicPacket, packet::MAX_PLAYERS};", "use doom_net::{TicPacket, packet::MAX_PLAYERS};\n/// use doom_types::TicCmd;")
with open('crates/doom-net/src/rollback.rs', 'w') as f:
    f.write(data)
