import re

with open('crates/doom-game/src/dehacked.rs', 'r') as f:
    content = f.read()

# Replace `.parse::<f64>().map_err(|_| ())?`
content = content.replace('.parse::<f64>().map_err(|_| ())?', '.parse::<f64>().map_err(|_| DehError::BadField(format!("expected valid number, got {s:?}")))?')

# We also need to fix the return type of the closure passed to `or_else` which is currently returning `Result<T, ()>` instead of `Result<T, DehError>`.
content = content.replace('Err(())', 'Err(DehError::BadField(format!("expected valid number, got {s:?}")))')

with open('crates/doom-game/src/dehacked.rs', 'w') as f:
    f.write(content)
