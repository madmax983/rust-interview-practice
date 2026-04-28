import re

with open("src/systems/raft.rs", "r") as f:
    content = f.read()

# Replace iterations over cloned peers
content = content.replace("for peer in self.peers.clone() {", "for &peer in &self.peers {")

with open("src/systems/raft.rs", "w") as f:
    f.write(content)
