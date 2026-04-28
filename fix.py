import sys

with open('crates/doom-map/src/analyzer.rs', 'r') as f:
    code = f.read()

# fix the borrow checker error
search_str2 = """                        // After visiting all neighbors of u, if u is not root, update parent's low_time
                        if let Some(&p) = parent.get(&u) {
                            let low_u = *low_time.get(&u).unwrap();
                            let low_p = *low_time.get(&p).unwrap();
                            low_time.insert(p, low_p.min(low_u));

                            let disc_p = *discovery_time.get(&p).unwrap();
                            if low_u >= disc_p && parent.contains_key(&p) {
                                articulation_points.insert(p);
                            }
                        } else if *children_map.get(&u).unwrap_or(&0) > 1 {
                            articulation_points.insert(u);
                        }"""

replace_str2 = """                        // After visiting all neighbors of u, if u is not root, update parent's low_time
                        if let Some(&p) = parent.get(&u) {
                            let (low_u, low_p, disc_p) = (
                                low_time.get(&u).copied(),
                                low_time.get(&p).copied(),
                                discovery_time.get(&p).copied(),
                            );
                            if let (Some(low_u), Some(low_p), Some(disc_p)) = (low_u, low_p, disc_p) {
                                let new_low = low_p.min(low_u);
                                low_time.insert(p, new_low);

                                if low_u >= disc_p && parent.contains_key(&p) {
                                    articulation_points.insert(p);
                                }
                            }
                        } else if *children_map.get(&u).unwrap_or(&0) > 1 {
                            articulation_points.insert(u);
                        }"""

code = code.replace(search_str2, replace_str2)

search_str3 = """                        } else if parent.get(&u) != Some(&v) {
                            let low_u = *low_time.get(&u).unwrap();
                            let disc_v = *discovery_time.get(&v).unwrap();
                            low_time.insert(u, low_u.min(disc_v));
                        }"""

replace_str3 = """                        } else if parent.get(&u) != Some(&v) {
                            let (low_u, disc_v) = (
                                low_time.get(&u).copied(),
                                discovery_time.get(&v).copied(),
                            );
                            if let (Some(low_u), Some(disc_v)) = (low_u, disc_v) {
                                let new_low = low_u.min(disc_v);
                                low_time.insert(u, new_low);
                            }
                        }"""

code = code.replace(search_str3, replace_str3)

with open('crates/doom-map/src/analyzer.rs', 'w') as f:
    f.write(code)
