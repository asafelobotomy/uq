//! Adding a printer: looking for printers nearby, offering drivers, and adding the queue.

use super::*;

impl PrintersState {
    /// Look for printers in the background; the result lands in `found`.
    pub(crate) fn look(&self) {
        if self.shared.looking.swap(true, Ordering::SeqCst) {
            return;
        }
        let cups = self.cups.clone();
        let shared = self.shared.clone();
        let task = async move {
            let mut r = cups.request(op::CUPS_GET_DEVICES);
            r.op_attr("timeout", Out::Int(LOOK_SECONDS))
                .op_attrs("exclude-schemes", vec![str_out(tag::NAME, "beh")]);
            let wait = Duration::from_secs(u64::try_from(LOOK_SECONDS).unwrap_or(8) + 20);
            match cups.send("/admin/", &r, wait).await {
                Ok(resp) => {
                    let mut seen: Vec<String> = Vec::new();
                    let mut list = Vec::new();
                    for d in resp.groups(tag::PRINTER) {
                        let uri = d.text("device-uri");
                        // A bare scheme ("ipp", "socket") is a way to add one by hand, not a
                        // printer that was found.
                        if !uri.contains(':') || seen.contains(&uri) {
                            continue;
                        }
                        seen.push(uri.clone());
                        let model = d.text("device-make-and-model");
                        let model = if model == "Unknown" {
                            String::new()
                        } else {
                            model
                        };
                        let info = d.text("device-info");
                        list.push(json!({
                            "uri": uri,
                            "info": if info.is_empty() { model.clone() } else { info },
                            "model": model,
                            "device_id": d.text("device-id"),
                            "location": d.text("device-location"),
                            "connection": connection_words(&uri),
                            "driverless": driverless(&uri),
                        }));
                    }
                    list.sort_by_key(|d| {
                        (
                            d["driverless"] != true,
                            d["info"].as_str().unwrap_or("").to_lowercase(),
                        )
                    });
                    *shared.found.lock().unwrap() = Some(list);
                }
                Err(e) => shared.notice(&format!("Looking for printers didn't work: {}", e.text())),
            }
            shared.looking.store(false, Ordering::SeqCst);
            shared.ping();
        };
        let mut tasks = self.tasks.lock().unwrap();
        tasks.retain(|t| !t.is_finished());
        tasks.push(tokio::spawn(task));
    }

    pub(crate) async fn drivers(
        &self,
        uri: &str,
        device_id: &str,
        make_model: &str,
    ) -> zbus::Result<()> {
        let mut list = Vec::new();
        if driverless(uri) {
            list.push(json!({"name": "everywhere", "label": "Driverless (recommended)"}));
        }
        {
            let mut r = self.cups.request(op::CUPS_GET_PPDS);
            r.op_attr("limit", Out::Int(DRIVER_LIMIT)).op_attrs(
                "requested-attributes",
                keywords(&["ppd-name", "ppd-make-and-model", "ppd-natural-language"]),
            );
            if !device_id.is_empty() {
                r.op_attr("ppd-device-id", str_out(tag::TEXT, device_id));
            } else if !make_model.is_empty() {
                r.op_attr("ppd-make-and-model", str_out(tag::TEXT, make_model));
            }
            if !device_id.is_empty() || !make_model.is_empty() {
                let resp = self
                    .cups
                    .send("/", &r, CHANGE_TIMEOUT)
                    .await
                    .map_err(shown)?;
                for d in resp.groups(tag::PRINTER) {
                    let name = d.text("ppd-name");
                    if name.is_empty() || name == "everywhere" {
                        continue;
                    }
                    let lang = d.text("ppd-natural-language");
                    if !lang.is_empty() && lang != "en" && !lang.starts_with("en") {
                        continue;
                    }
                    list.push(json!({"name": name, "label": d.text("ppd-make-and-model")}));
                }
            }
        }
        *self.shared.drivers.lock().unwrap() = json!({"uri": uri, "drivers": list});
        Ok(())
    }

    pub(crate) async fn add(
        &self,
        uri: &str,
        name: &str,
        location: &str,
        driver: &str,
    ) -> zbus::Result<()> {
        let uri = uri.trim();
        if uri.is_empty() || !uri.contains(':') {
            return Err(failure("Give the printer's address."));
        }
        if driver.is_empty() {
            return Err(failure("Choose a driver for this printer."));
        }
        let printers = self.printers().await.map_err(shown)?;
        let taken: Vec<String> = printers.iter().map(|p| p.text("printer-name")).collect();
        let queue = queue_name(name, &taken);
        let info = if name.trim().is_empty() {
            queue.clone()
        } else {
            name.trim().to_string()
        };
        let mut r = self.printer_request(op::CUPS_ADD_MODIFY_PRINTER, &queue);
        r.printer_attr("device-uri", str_out(tag::URI, uri))
            .printer_attr("printer-info", str_out(tag::TEXT, &info))
            .printer_attr("printer-location", str_out(tag::TEXT, location.trim()))
            .printer_attr("ppd-name", str_out(tag::NAME, driver))
            .printer_attr("printer-is-accepting-jobs", Out::Bool(true))
            .printer_attr("printer-state", Out::Enum(3));
        self.admin(&r).await?;
        // The first printer becomes the default, as people expect.
        if self.default_name().await.ok().flatten().is_none() {
            let r = self.printer_request(op::CUPS_SET_DEFAULT, &queue);
            self.admin(&r).await?;
        }
        self.shared.notice(&format!("{info} was added."));
        Ok(())
    }
}
