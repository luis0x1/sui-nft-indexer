use chrono::{ Utc, TimeZone };

pub fn millis_to_iso(millis: u64) -> String {
  // Convert milliseconds to a DateTime<Utc>
  let datetime = Utc.timestamp_millis_opt(millis as i64).unwrap();
  // Format as ISO 8601 with milliseconds and 'Z'
  datetime.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}
