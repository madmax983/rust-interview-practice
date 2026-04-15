import re

with open("src/systems/cron.rs", "r") as f:
    content = f.read()

content = content.replace("#[derive(Debug, Clone, PartialEq, Eq)]\npub trait Schedule", "pub trait Schedule")
content = content.replace("/// Represents an optimized Cron Schedule.\npub struct CronSchedule", "/// Represents an optimized Cron Schedule.\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct CronSchedule")

with open("src/systems/cron.rs", "w") as f:
    f.write(content)
