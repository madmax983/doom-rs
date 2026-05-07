with open("crates/doom-map/src/analyzer.rs", "r") as f:
    content = f.read()

s1 = """                        } else if parent.get(&u) != Some(&v) {
                            let (low_u, disc_v) =
                                (low_time.get(&u).copied(), discovery_time.get(&v).copied());
                            if let (Some(low_u), Some(disc_v)) = (low_u, disc_v) {
                                let new_low = low_u.min(disc_v);
                                low_time.insert(u, new_low);
                            }
                        }"""
r1 = """                        } else if parent.get(&u) != Some(&v) {
                            if let (Some(low_u), Some(disc_v)) =
                                (low_time.get(&u).copied(), discovery_time.get(&v).copied())
                            {
                                let new_low = low_u.min(disc_v);
                                low_time.insert(u, new_low);
                            }
                        }"""
content = content.replace(s1, r1)

s2 = """                        if let Some(&p) = parent.get(&u) {
                            let (low_u, low_p, disc_p) = (
                                low_time.get(&u).copied(),
                                low_time.get(&p).copied(),
                                discovery_time.get(&p).copied(),
                            );
                            if let (Some(low_u), Some(low_p), Some(disc_p)) = (low_u, low_p, disc_p)
                            {
                                let new_low = low_p.min(low_u);
                                low_time.insert(p, new_low);

                                if low_u >= disc_p && parent.contains_key(&p) {
                                    articulation_points.insert(p);
                                }
                            }
                        }"""
r2 = """                        if let Some(&p) = parent.get(&u) {
                            if let (Some(low_u), Some(low_p), Some(disc_p)) = (
                                low_time.get(&u).copied(),
                                low_time.get(&p).copied(),
                                discovery_time.get(&p).copied(),
                            ) {
                                let new_low = low_p.min(low_u);
                                low_time.insert(p, new_low);

                                if low_u >= disc_p && parent.contains_key(&p) {
                                    articulation_points.insert(p);
                                }
                            }
                        }"""
content = content.replace(s2, r2)

with open("crates/doom-map/src/analyzer.rs", "w") as f:
    f.write(content)
