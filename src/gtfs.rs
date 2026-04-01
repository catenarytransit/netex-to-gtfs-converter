use serde::Serialize;

#[derive(Serialize)]
pub struct Agency {
    pub agency_id: String,
    pub agency_name: String,
    pub agency_url: String,
    pub agency_timezone: String,
    pub agency_lang: String,
}

#[derive(Serialize)]
pub struct Stop {
    pub stop_id: String,
    pub stop_name: String,
    pub stop_lat: f64,
    pub stop_lon: f64,
    pub location_type: Option<u8>,
    pub parent_station: Option<String>,
}

#[derive(Serialize)]
pub struct Route {
    pub route_id: String,
    pub agency_id: Option<String>,
    pub route_short_name: String,
    pub route_long_name: String,
    pub route_type: u8,
}

#[derive(Serialize)]
pub struct Trip {
    pub route_id: String,
    pub service_id: String,
    pub trip_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trip_short_name: Option<String>,
}

#[derive(Serialize)]
pub struct StopTime {
    pub trip_id: String,
    pub arrival_time: String,
    pub departure_time: String,
    pub stop_id: String,
    pub stop_sequence: u32,
}

#[derive(Serialize)]
pub struct Calendar {
    pub service_id: String,
    pub monday: u8,
    pub tuesday: u8,
    pub wednesday: u8,
    pub thursday: u8,
    pub friday: u8,
    pub saturday: u8,
    pub sunday: u8,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Serialize)]
pub struct CalendarDate {
    pub service_id: String,
    pub date: String,
    pub exception_type: u8,
}
