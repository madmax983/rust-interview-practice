sed -i 's/pub fn matches/fn matches/' src/systems/cron.rs
sed -i 's/    #\[must_use\]//' src/systems/cron.rs
sed -i '/#\[derive(Debug, Clone, PartialEq, Eq)\]/{
N
N
N
N
N
N
N
N
N
N
N
s/#\[derive(Debug, Clone, PartialEq, Eq)\]\npub trait Schedule {\n    fn matches(\n        &self,\n        minute: u8,\n        hour: u8,\n        day_of_month: u8,\n        month: u8,\n        day_of_week: u8,\n    ) -> bool;\n}\n\n\/\/\/ Represents an optimized Cron Schedule.\npub struct CronSchedule/pub trait Schedule {\n    fn matches(\n        \&self,\n        minute: u8,\n        hour: u8,\n        day_of_month: u8,\n        month: u8,\n        day_of_week: u8,\n    ) -> bool;\n}\n\n\/\/\/ Represents an optimized Cron Schedule.\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct CronSchedule/
}' src/systems/cron.rs
