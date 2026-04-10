with open("crates/doom-app/src/main.rs", "r") as f:
    text = f.read()

print(f"Number of 'MenuAction::Noop' in main.rs: {text.count('MenuAction::Noop')}")
