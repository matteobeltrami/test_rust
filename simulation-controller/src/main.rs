mod utils;

slint::include_modules!();

use std::rc::Rc;

use chrono::{Datelike, Local, Timelike};
use slint::{Model, ModelRc, SharedString, VecModel};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let main_window = MainWindow::new()?;

    // Drones
    let drones_model: Rc<VecModel<Drone>> = Rc::new(VecModel::from(vec![
        Drone { title: "Drone 1".into(), id: "1".into() },
        Drone { title: "Drone 2".into(), id: "2".into() },
    ]));
    main_window.set_drones(ModelRc::from(drones_model.clone()));

    // Clients
    let clients_model: Rc<VecModel<Client>> = Rc::new(VecModel::from(vec![
        Client { title: "Client 1".into(), subtitle: "Web Client".into(), id: "3".into() },
        Client { title: "Client 2".into(), subtitle: "Chat Client".into(), id: "4".into() },
    ]));
    main_window.set_clients(ModelRc::from(clients_model.clone()));

    // Servers
    let servers_model: Rc<VecModel<Server>> = Rc::new(VecModel::from(vec![
        Server { title: "Server 1".into(), id: "5".into() },
        Server { title: "Server 2".into(), id: "6".into() },
    ]));
    main_window.set_servers(ModelRc::from(servers_model.clone()));

    // Current PDR
    main_window.set_initial_pdr(SharedString::from("0"));

    main_window.on_validate_pdr(|input: SharedString, current: SharedString| -> SharedString {
        utils::validate_pdr(&input, &current).into()
    });

    let logs_model: Rc<VecModel<SharedString>> =Rc::new(VecModel::from(Vec::<SharedString>::new()));
    main_window.set_logs(logs_model.clone().into());

    // The .slint callback: when UI calls add_log(...), just append the given line (no formatting here).
    {
        let logs_model = logs_model.clone();
        main_window.on_add_log(move |line: SharedString| {
            logs_model.push(line);
        });
    }

    // Register a global logger in utils that formats a timestamp and calls into the UI via invoke_add_log
    {
        let mw_weak = main_window.as_weak();
        utils::set_logger(Box::new(move |msg: SharedString| {
            let _ = slint::invoke_from_event_loop({
                let mw_weak = mw_weak.clone();
                move || {
                    if let Some(mw) = mw_weak.upgrade() {
                        let now = Local::now();
                        // Example format: [22/08/2025 14:35:12] message
                        let formatted = format!(
                            "[{}/{}/{} {:02}:{:02}:{:02}] {}",
                            now.day(),
                            now.month(),
                            now.year(),
                            now.hour(),
                            now.minute(),
                            now.second(),
                            msg
                        );
                        mw.invoke_add_log(formatted.into());
                    }
                }
            });
        }));
    }

    // Handlers that change the graph after removing/crashing nodes
    {
        let mw_weak = main_window.as_weak();
        let drones_model = drones_model.clone();
        let clients_model = clients_model.clone();
        let servers_model = servers_model.clone();

        main_window.on_remove_node(move |input: SharedString| {
            if let Some(pos) = (0..drones_model.row_count())
                .position(|i| drones_model
                    .row_data(i)
                    .map(|d| d.id == input)
                    .unwrap_or(false))
            {
                utils::remove_node(&input);
                drones_model.remove(pos);

                if let Some(mw) = mw_weak.upgrade() {
                    mw.set_graph_image(utils::update_graph(
                        drones_model.clone(),
                        clients_model.clone(),
                        servers_model.clone(),
                    ));
                }
            }
        });
    }

    {
        let mw_weak = main_window.as_weak();
        let drones_model = drones_model.clone();
        let clients_model = clients_model.clone();
        let servers_model = servers_model.clone();

        main_window.on_crash_node(move |input: SharedString| {
            if let Some(pos) = (0..drones_model.row_count())
                .position(|i| drones_model
                    .row_data(i)
                    .map(|d| d.id == input)
                    .unwrap_or(false))
            {
                utils::crash_node(&input);
                drones_model.remove(pos);

                if let Some(mw) = mw_weak.upgrade() {
                    mw.set_graph_image(utils::update_graph(
                        drones_model.clone(),
                        clients_model.clone(),
                        servers_model.clone(),
                    ));
                }
            }
        });
    }

    utils::log("Simulation Controller started");

    main_window.run()?;
    Ok(())
}
