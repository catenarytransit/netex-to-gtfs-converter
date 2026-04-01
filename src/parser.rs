use crate::gtfs::{Calendar, CalendarDate, Route, Stop, StopTime, Trip};
use quick_xml::events::Event;
use quick_xml::Reader;
use rustc_hash::{FxHashMap, FxHashSet};
use std::fs;
use std::fs::File;
use std::path::Path;

pub struct GtfsModel {
    pub stops: Vec<Stop>,
    pub routes: Vec<Route>,
    pub trips: Vec<Trip>,
    pub stop_times: Vec<StopTime>,
    pub calendars: Vec<Calendar>,
    pub calendar_dates: Vec<CalendarDate>,
}

#[derive(Default, Debug)]
struct Call {
    seq: u32,
    quay_ref: Option<String>,
    stop_point_ref: Option<String>,
    spijp_ref: Option<String>,
    arr_time: Option<String>,
    dep_time: Option<String>,
}

pub fn parse_netex(path: &Path) -> Result<GtfsModel, Box<dyn std::error::Error>> {
    let mut reader = Reader::from_file(path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut path_stack = Vec::new();

    let mut stops = Vec::new();
    let mut routes = Vec::new();
    let mut trips = Vec::new();
    let mut stop_times = Vec::new();
    let mut calendars = Vec::new();
    let mut calendar_dates = Vec::new();

    // Map Quay -> parent StopPlace
    let mut quay_to_parent: FxHashMap<String, String> = FxHashMap::default();
    
    // State variables
    let mut current_stop_place_id = String::new();
    let mut current_stop_place_name = String::new();
    let mut current_stop_place_lat = 0.0;
    let mut current_stop_place_lon = 0.0;

    let mut current_quay_id = String::new();
    let mut current_quay_name = String::new();

    // ScheduledStopPoint state (for stop_ids used in stop_times)
    let mut current_sched_stop_id = String::new();
    let mut current_sched_stop_name = String::new();
    let mut current_sched_stop_lat = 0.0;
    let mut current_sched_stop_lon = 0.0;
    
    let mut current_line_id = String::new();
    let mut current_line_name = String::new();

    let mut current_spijp_id = String::new();
    let mut spijp_to_stop: FxHashMap<String, String> = FxHashMap::default();

    let mut current_vj_id = String::new();
    let mut current_vj_line_ref = String::new();
    let mut current_vj_service_id = String::new();
    let mut current_vj_name = String::new();
    
    let mut current_calls: Vec<Call> = Vec::new();
    let mut current_call = Call::default();

    // DayType and operating period state
    let mut current_day_type_id = String::new();
    let mut current_day_type_weekdays = [0u8; 7];
    let mut day_type_weekdays: FxHashMap<String, [u8; 7]> = FxHashMap::default();

    let mut current_uic_id = String::new();
    let mut current_uic_from = String::new();
    let mut current_uic_to = String::new();
    let mut uic_periods: FxHashMap<String, (String, String)> = FxHashMap::default();

    // Track which service_ids actually appear in trips
    let mut used_service_ids: FxHashSet<String> = FxHashSet::default();

    let mut text_buf = String::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => {
                let name = std::str::from_utf8(e.name().into_inner())?.to_string();
                path_stack.push(name.clone());
                text_buf.clear();

                let get_attr = |key: &str| -> Option<String> {
                    e.attributes()
                        .filter_map(Result::ok)
                        .find(|a| a.key.into_inner() == key.as_bytes())
                        .and_then(|a| std::str::from_utf8(&a.value).ok().map(|s| s.to_string()))
                };

                match name.as_str() {
                    "DayType" => {
                        if let Some(id) = get_attr("id") {
                            current_day_type_id = id;
                            current_day_type_weekdays = [0u8; 7];
                        }
                    }
                    "UicOperatingPeriod" => {
                        if let Some(id) = get_attr("id") {
                            current_uic_id = id;
                            current_uic_from.clear();
                            current_uic_to.clear();
                        }
                    }
                    "ScheduledStopPoint" => {
                        if let Some(id) = get_attr("id") {
                            current_sched_stop_id = id;
                            current_sched_stop_name.clear();
                            current_sched_stop_lat = 0.0;
                            current_sched_stop_lon = 0.0;
                        }
                    }
                    "StopPlace" => {
                        if let Some(id) = get_attr("id") {
                            current_stop_place_id = id;
                        }
                    }
                    "Quay" => {
                        if let Some(id) = get_attr("id") {
                            current_quay_id = id;
                            current_quay_name = current_stop_place_name.clone(); // fallback
                        }
                    }
                    "Line" => {
                        if let Some(id) = get_attr("id") {
                            current_line_id = id;
                        }
                    }
                    "ServiceJourney" | "VehicleJourney" => {
                        if let Some(id) = get_attr("id") {
                            current_vj_id = id;
                            current_vj_line_ref.clear();
                            current_vj_service_id = "default".to_string();
                            current_vj_name.clear();
                            current_calls.clear();
                        }
                    }
                    "StopPointInJourneyPattern" => {
                        if let Some(id) = get_attr("id") {
                            current_spijp_id = id;
                        }
                    }
                    "Call" | "TimetabledPassingTime" | "TargetPassingTime" => {
                        current_call = Call::default();
                        if let Some(order) = get_attr("order") {
                            current_call.seq = order.parse().unwrap_or(0);
                        } else {
                            current_call.seq = current_calls.len() as u32 + 1;
                        }
                    }
                    "ScheduledStopPointRef" | "StopPointRef" => {
                        if let Some(ref_id) = get_attr("ref") {
                            let parent = if path_stack.len() > 1 { path_stack[path_stack.len() - 2].as_str() } else { "" };
                            if parent == "StopPointInJourneyPattern" {
                                spijp_to_stop.insert(current_spijp_id.clone(), ref_id);
                            } else {
                                current_call.stop_point_ref = Some(ref_id);
                            }
                        }
                    }
                    "QuayRef" => {
                        if let Some(ref_id) = get_attr("ref") {
                            let parent = if path_stack.len() > 1 { path_stack[path_stack.len() - 2].as_str() } else { "" };
                            if parent == "StopPointInJourneyPattern" {
                                spijp_to_stop.insert(current_spijp_id.clone(), ref_id);
                            } else {
                                current_call.quay_ref = Some(ref_id);
                            }
                        }
                    }
                    "StopPointInJourneyPatternRef" => {
                        if let Some(ref_id) = get_attr("ref") {
                            current_call.spijp_ref = Some(ref_id);
                        }
                    }
                    "LineRef" => {
                        if let Some(ref_id) = get_attr("ref") {
                            current_vj_line_ref = ref_id;
                        }
                    }
                    "OperatingPeriodRef" | "DayTypeRef" => {
                        if let Some(ref_id) = get_attr("ref") {
                            current_vj_service_id = ref_id;
                        }
                    }
                    _ => {}
                }
            }
            Event::Empty(ref e) => {
                let name = std::str::from_utf8(e.name().into_inner())?.to_string();
                let get_attr = |key: &str| -> Option<String> {
                    e.attributes()
                        .filter_map(Result::ok)
                        .find(|a| a.key.into_inner() == key.as_bytes())
                        .and_then(|a| std::str::from_utf8(&a.value).ok().map(|s| s.to_string()))
                };

                let parent = path_stack.last().map(|s| s.as_str()).unwrap_or("");
                match name.as_str() {
                    "ScheduledStopPointRef" | "StopPointRef" => {
                        if let Some(ref_id) = get_attr("ref") {
                            if parent == "StopPointInJourneyPattern" {
                                spijp_to_stop.insert(current_spijp_id.clone(), ref_id);
                            } else {
                                current_call.stop_point_ref = Some(ref_id);
                            }
                        }
                    }
                    "QuayRef" => {
                        if let Some(ref_id) = get_attr("ref") {
                            if parent == "StopPointInJourneyPattern" {
                                spijp_to_stop.insert(current_spijp_id.clone(), ref_id);
                            } else {
                                current_call.quay_ref = Some(ref_id);
                            }
                        }
                    }
                    "StopPointInJourneyPatternRef" => {
                        if let Some(ref_id) = get_attr("ref") {
                            current_call.spijp_ref = Some(ref_id);
                        }
                    }
                    "LineRef" => {
                        if let Some(ref_id) = get_attr("ref") {
                            current_vj_line_ref = ref_id;
                        }
                    }
                    "OperatingPeriodRef" | "DayTypeRef" => {
                        if let Some(ref_id) = get_attr("ref") {
                            current_vj_service_id = ref_id;
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(e) => {
                text_buf = e.unescape()?.to_string();
            }
            Event::End(ref e) => {
                let name = std::str::from_utf8(e.name().into_inner())?.to_string();
                
                // Track where we are
                let parent = if path_stack.len() > 1 { path_stack[path_stack.len() - 2].as_str() } else { "" };
                let gparent = if path_stack.len() > 2 { path_stack[path_stack.len() - 3].as_str() } else { "" };
                let ggparent = if path_stack.len() > 3 { path_stack[path_stack.len() - 4].as_str() } else { "" };

                match name.as_str() {
                    "DaysOfWeek" => {
                        if gparent == "DayType" {
                            for token in text_buf.split_whitespace() {
                                let idx = match token {
                                    "Monday" => Some(0),
                                    "Tuesday" => Some(1),
                                    "Wednesday" => Some(2),
                                    "Thursday" => Some(3),
                                    "Friday" => Some(4),
                                    "Saturday" => Some(5),
                                    "Sunday" => Some(6),
                                    _ => None,
                                };
                                if let Some(i) = idx {
                                    current_day_type_weekdays[i] = 1;
                                }
                            }
                        }
                    }
                    "Name" => {
                        if parent == "StopPlace" {
                            current_stop_place_name = text_buf.clone();
                        } else if parent == "Quay" {
                            current_quay_name = text_buf.clone();
                        } else if parent == "ScheduledStopPoint" {
                            current_sched_stop_name = text_buf.clone();
                        } else if parent == "Line" {
                            current_line_name = text_buf.clone();
                        } else if parent == "ServiceJourney" || parent == "VehicleJourney" {
                            current_vj_name = text_buf.clone();
                        }
                    }
                    "Longitude" => {
                        if let Ok(v) = text_buf.parse::<f64>() {
                            if parent == "Location" && gparent == "Centroid" && ggparent == "StopPlace" {
                                current_stop_place_lon = v;
                            } else if parent == "Location" && gparent == "ScheduledStopPoint" {
                                current_sched_stop_lon = v;
                            }
                        }
                    }
                    "Latitude" => {
                        if let Ok(v) = text_buf.parse::<f64>() {
                            if parent == "Location" && gparent == "Centroid" && ggparent == "StopPlace" {
                                current_stop_place_lat = v;
                            } else if parent == "Location" && gparent == "ScheduledStopPoint" {
                                current_sched_stop_lat = v;
                            }
                        }
                    }
                    "FromDate" => {
                        if parent == "UicOperatingPeriod" {
                            current_uic_from = text_buf.clone();
                        }
                    }
                    "ToDate" => {
                        if parent == "UicOperatingPeriod" {
                            current_uic_to = text_buf.clone();
                        }
                    }
                    "Time" => {
                        if parent == "Arrival" && gparent == "Call" {
                            current_call.arr_time = Some(text_buf.clone());
                        } else if parent == "Departure" && gparent == "Call" {
                            current_call.dep_time = Some(text_buf.clone());
                        }
                    }
                    "ArrivalTime" | "TargetArrivalTime" | "TimetabledArrivalTime" => {
                        if parent == "TimetabledPassingTime" || parent == "Call" || parent == "TargetPassingTime" {
                            current_call.arr_time = Some(text_buf.clone());
                        }
                    }
                    "DepartureTime" | "TargetDepartureTime" | "TimetabledDepartureTime" => {
                        if parent == "TimetabledPassingTime" || parent == "Call" || parent == "TargetPassingTime" {
                            current_call.dep_time = Some(text_buf.clone());
                        }
                    }
                    "ScheduledStopPoint" => {
                        if !current_sched_stop_id.is_empty() {
                            let parent_station = if current_sched_stop_id.contains("ScheduledStopPoint:") {
                                Some(current_sched_stop_id.replace("ScheduledStopPoint:", "StopPlace:"))
                            } else {
                                None
                            };

                            stops.push(Stop {
                                stop_id: current_sched_stop_id.clone(),
                                stop_name: current_sched_stop_name.clone(),
                                stop_lat: current_sched_stop_lat,
                                stop_lon: current_sched_stop_lon,
                                // Treat ScheduledStopPoint as a regular stop/platform
                                location_type: Some(0),
                                parent_station,
                            });
                        }
                        current_sched_stop_id.clear();
                        current_sched_stop_name.clear();
                        current_sched_stop_lat = 0.0;
                        current_sched_stop_lon = 0.0;
                    }
                    "StopPlace" => {
                        stops.push(Stop {
                            stop_id: current_stop_place_id.clone(),
                            stop_name: current_stop_place_name.clone(),
                            stop_lat: current_stop_place_lat,
                            stop_lon: current_stop_place_lon,
                            location_type: Some(1),
                            parent_station: None,
                        });
                        current_stop_place_name.clear();
                        current_stop_place_lat = 0.0;
                        current_stop_place_lon = 0.0;
                    }
                    "DayType" => {
                        if !current_day_type_id.is_empty() {
                            day_type_weekdays.insert(current_day_type_id.clone(), current_day_type_weekdays);
                        }
                        current_day_type_id.clear();
                    }
                    "UicOperatingPeriod" => {
                        if !current_uic_id.is_empty() && !current_uic_from.is_empty() && !current_uic_to.is_empty() {
                            let to_ymd = |s: &str| -> String {
                                let s = s.trim();
                                let date_part = s.split('T').next().unwrap_or(s);
                                date_part.chars().filter(|c| *c != '-').collect()
                            };
                            let start = to_ymd(&current_uic_from);
                            let end = to_ymd(&current_uic_to);
                            uic_periods.insert(current_uic_id.clone(), (start, end));
                        }
                        current_uic_id.clear();
                        current_uic_from.clear();
                        current_uic_to.clear();
                    }
                    "Quay" => {
                        stops.push(Stop {
                            stop_id: current_quay_id.clone(),
                            stop_name: current_quay_name.clone(),
                            stop_lat: current_stop_place_lat, // fallback to stop place location
                            stop_lon: current_stop_place_lon,
                            location_type: Some(0),
                            parent_station: Some(current_stop_place_id.clone()),
                        });
                        quay_to_parent.insert(current_quay_id.clone(), current_stop_place_id.clone());
                        current_quay_name.clear();
                    }
                    "Line" => {
                        if !current_line_id.is_empty() {
                            routes.push(Route {
                                route_id: current_line_id.clone(),
                                agency_id: None,
                                route_short_name: current_line_name.clone(),
                                route_long_name: current_line_name.clone(),
                                route_type: 2, // Rail
                            });
                        }
                    }
                    "Call" | "TimetabledPassingTime" | "TargetPassingTime" => {
                        current_calls.push(std::mem::take(&mut current_call));
                    }
                    "ServiceJourney" | "VehicleJourney" => {
                        // Resolve synthesized route
                        let synthetic_route_id = if current_vj_line_ref.is_empty() {
                            if let (Some(first), Some(last)) = (current_calls.first(), current_calls.last()) {
                                let first_ref = first.quay_ref.as_ref().or(first.stop_point_ref.as_ref())
                                    .or_else(|| first.spijp_ref.as_ref().and_then(|r| spijp_to_stop.get(r)));
                                let start = first_ref.unwrap_or(&"".to_string()).clone();
                                
                                let last_ref = last.quay_ref.as_ref().or(last.stop_point_ref.as_ref())
                                    .or_else(|| last.spijp_ref.as_ref().and_then(|r| spijp_to_stop.get(r)));
                                let end = last_ref.unwrap_or(&"".to_string()).clone();
                                
                                let start_parent = quay_to_parent.get(&start).unwrap_or(&start).clone();
                                let end_parent = quay_to_parent.get(&end).unwrap_or(&end).clone();
                                
                                let r_id = format!("{}_to_{}", start_parent, end_parent);
                                
                                // Add route if missing
                                if !routes.iter().any(|r| r.route_id == r_id) {
                                    routes.push(Route {
                                        route_id: r_id.clone(),
                                        agency_id: None,
                                        route_short_name: format!("{} to {}", start_parent, end_parent),
                                        route_long_name: format!("{} to {}", start_parent, end_parent),
                                        route_type: 2,
                                    });
                                }
                                r_id
                            } else {
                                "unknown_route".to_string()
                            }
                        } else {
                            current_vj_line_ref.clone()
                        };

                        if !current_vj_id.is_empty() {
                            trips.push(Trip {
                                route_id: synthetic_route_id,
                                service_id: current_vj_service_id.clone(),
                                trip_id: current_vj_id.clone(),
                                trip_short_name: if current_vj_name.is_empty() { None } else { Some(current_vj_name.clone()) },
                            });

                            if !current_vj_service_id.is_empty() {
                                used_service_ids.insert(current_vj_service_id.clone());
                            }

                            let mut prev_time = "00:00:00".to_string();
                            for call in &current_calls {
                                let mut arr = call.arr_time.clone().unwrap_or_else(|| prev_time.clone());
                                let mut dep = call.dep_time.clone().unwrap_or_else(|| arr.clone());
                                
                                // Clean up times from isoformat (e.g. 15:30:00 or 15:30 or 2026-..T15:30:00)
                                if arr.len() > 8 && arr.contains('T') { arr = arr.split('T').last().unwrap().to_string(); }
                                if dep.len() > 8 && dep.contains('T') { dep = dep.split('T').last().unwrap().to_string(); }
                                
                                if arr.is_empty() { arr = "00:00:00".to_string(); }
                                if dep.is_empty() { dep = "00:00:00".to_string(); }

                                prev_time = dep.clone();
                                
                                let stop_id = call.quay_ref.as_ref()
                                    .or(call.stop_point_ref.as_ref())
                                    .or_else(|| call.spijp_ref.as_ref().and_then(|r| spijp_to_stop.get(r)))
                                    .unwrap_or(&"unknown".to_string()).clone();

                                stop_times.push(StopTime {
                                    trip_id: current_vj_id.clone(),
                                    arrival_time: arr,
                                    departure_time: dep,
                                    stop_id,
                                    stop_sequence: call.seq,
                                });
                            }
                        }
                    }
                    _ => {}
                }
                path_stack.pop();
            }
            Event::Eof => break,
            _ => (),
        }
        buf.clear();
    }
    
    // Build calendars based on used service_ids, DayType and UicOperatingPeriod
    for service_id in used_service_ids {
        let weekdays = day_type_weekdays
            .get(&service_id)
            .copied()
            .unwrap_or([1, 1, 1, 1, 1, 1, 1]);

        // Try to infer corresponding UicOperatingPeriod id from DayType id
        let mut period_id = String::new();
        if service_id.contains("DayType:") {
            period_id = service_id.replace("DayType:", "UicOperatingPeriod:");
        }
        // Also allow direct use if service_id itself is a UicOperatingPeriod id
        let period_key = if uic_periods.contains_key(&service_id) {
            service_id.clone()
        } else {
            period_id.clone()
        };

        let (start_date, end_date) = if let Some((s, e)) = uic_periods.get(&period_key) {
            (s.clone(), e.clone())
        } else {
            // Fallback wide range if no specific period is found
            ("20240101".to_string(), "20261231".to_string())
        };

        calendars.push(Calendar {
            service_id: service_id.clone(),
            monday: weekdays[0],
            tuesday: weekdays[1],
            wednesday: weekdays[2],
            thursday: weekdays[3],
            friday: weekdays[4],
            saturday: weekdays[5],
            sunday: weekdays[6],
            start_date,
            end_date,
        });
    }

    // Keep a default calendar only if none could be constructed
    if calendars.is_empty() {
        calendars.push(Calendar {
            service_id: "default".to_string(),
            monday: 1, tuesday: 1, wednesday: 1, thursday: 1, friday: 1, saturday: 1, sunday: 1,
            start_date: "20240101".to_string(),
            end_date: "20261231".to_string(),
        });
    }

    Ok(GtfsModel { stops, routes, trips, stop_times, calendars, calendar_dates })
}

pub fn export_gtfs(model: &GtfsModel, out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(out_dir)?;

    let mut wtr = csv::Writer::from_path(out_dir.join("stops.txt"))?;
    for s in &model.stops { wtr.serialize(s)?; }
    wtr.flush()?;
    
    let mut wtr = csv::Writer::from_path(out_dir.join("routes.txt"))?;
    for r in &model.routes { wtr.serialize(r)?; }
    wtr.flush()?;
    
    let mut wtr = csv::Writer::from_path(out_dir.join("trips.txt"))?;
    for t in &model.trips { wtr.serialize(t)?; }
    wtr.flush()?;
    
    let mut wtr = csv::Writer::from_path(out_dir.join("stop_times.txt"))?;
    for st in &model.stop_times { wtr.serialize(st)?; }
    wtr.flush()?;
    
    let mut wtr = csv::Writer::from_path(out_dir.join("calendar.txt"))?;
    for c in &model.calendars { wtr.serialize(c)?; }
    wtr.flush()?;

    if !model.calendar_dates.is_empty() {
        let mut wtr = csv::Writer::from_path(out_dir.join("calendar_dates.txt"))?;
        for cd in &model.calendar_dates { wtr.serialize(cd)?; }
        wtr.flush()?;
    }

    Ok(())
}
