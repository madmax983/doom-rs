import re

with open('crates/doom-app/src/main.rs', 'r') as f:
    content = f.read()

search = """                        if is_tty {
                            println!(
                                "{} {} {}",
                                "🗺️ ".green(),
                                "Path found:".green().bold(),
                                path_str.cyan()
                            );
                        } else {
                            println!("Path found: {}", path_str);
                        }"""

replace = """                        if is_tty {
                            let mut table = comfy_table::Table::new();
                            table
                                .load_preset(comfy_table::presets::UTF8_FULL)
                                .apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS)
                                .set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
                            table.set_header(vec![
                                comfy_table::Cell::new("Feature")
                                    .fg(comfy_table::Color::Cyan)
                                    .add_attribute(comfy_table::Attribute::Bold),
                                comfy_table::Cell::new("Data")
                                    .fg(comfy_table::Color::Cyan)
                                    .add_attribute(comfy_table::Attribute::Bold),
                            ]);
                            table.add_row(vec![
                                comfy_table::Cell::new("🗺️  Path found:"),
                                comfy_table::Cell::new(&path_str).fg(comfy_table::Color::Green),
                            ]);
                            println!("{table}");
                        } else {
                            println!("Path found: {}", path_str);
                        }"""

new_content = content.replace(search, replace)
with open('crates/doom-app/src/main.rs', 'w') as f:
    f.write(new_content)
