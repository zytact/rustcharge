use battery::{Manager, State, units::Ratio};

pub fn get_battery_status() -> Result<(State, Ratio), battery::Error> {
    let manager = Manager::new()?;

    if let Some(Ok(battery)) = manager.batteries()?.next() {
        Ok((battery.state(), battery.state_of_charge()))
    } else {
        Err(battery::Error::from(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No battery found",
        )))
    }
}
