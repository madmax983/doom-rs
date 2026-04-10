with open(".jules/atlas.md", "r") as f:
    text = f.read()

text = text.replace("## YYYY-MM-DD - Move Skill enum\n**Tangle:**", "**[Move Skill enum]**\n**Tangle:**")
text = text.replace("## YYYY-MM-DD - Reduce pub visibility in binary\n**Tangle:**", "\n**[Reduce pub visibility in binary]**\n**Tangle:**")

with open(".jules/atlas.md", "w") as f:
    f.write(text)
