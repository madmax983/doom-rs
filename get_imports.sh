for file in $(find crates -name "*.rs"); do
  grep -Hn "doom_game::" "$file"
done
