//! Reading CUPS: the printers, their state and default, and their queues.

use super::*;

impl PrintersState {
    pub(crate) async fn printers(&self) -> Result<Vec<ipp::Group>, CupsError> {
        let mut r = self.cups.request(op::CUPS_GET_PRINTERS);
        r.op_attrs(
            "requested-attributes",
            keywords(&[
                "printer-name",
                "printer-info",
                "printer-location",
                "printer-make-and-model",
                "device-uri",
                "printer-state",
                "printer-state-reasons",
                "printer-state-message",
                "printer-is-accepting-jobs",
                "printer-type",
            ]),
        );
        let resp = self.cups.send("/", &r, READ_TIMEOUT).await?;
        Ok(resp.groups(tag::PRINTER).cloned().collect())
    }

    pub(crate) async fn default_name(&self) -> Result<Option<String>, CupsError> {
        let r = self.cups.request(op::CUPS_GET_DEFAULT);
        match self.cups.send("/", &r, READ_TIMEOUT).await {
            Ok(resp) => Ok(resp
                .groups(tag::PRINTER)
                .find_map(|g| g.str("printer-name").map(str::to_string))),
            // No default printer is "not found".
            Err(CupsError::Failed(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub(crate) async fn jobs(&self) -> Result<Vec<ipp::Group>, CupsError> {
        let mut r = self.cups.request(op::GET_JOBS);
        r.op_attr("printer-uri", str_out(tag::URI, "ipp://localhost/"))
            .op_attr("which-jobs", str_out(tag::KEYWORD, "not-completed"))
            .op_attr("my-jobs", Out::Bool(false))
            .op_attrs(
                "requested-attributes",
                keywords(&[
                    "job-id",
                    "job-name",
                    "job-state",
                    "job-originating-user-name",
                    "job-printer-uri",
                    "job-k-octets",
                ]),
            );
        let resp = self.cups.send("/", &r, READ_TIMEOUT).await?;
        Ok(resp.groups(tag::JOB).cloned().collect())
    }

    pub(crate) async fn read(&self) -> Result<Value, CupsError> {
        let printers = self.printers().await?;
        let default = self.default_name().await?;
        let jobs = self.jobs().await?;
        let mut by_printer: HashMap<String, Vec<Value>> = HashMap::new();
        for j in &jobs {
            let printer = j
                .text("job-printer-uri")
                .rsplit('/')
                .next()
                .map(|s| {
                    percent_encoding::percent_decode_str(s)
                        .decode_utf8_lossy()
                        .into_owned()
                })
                .unwrap_or_default();
            let owner = j.text("job-originating-user-name");
            let title = j.text("job-name");
            let state = j.int("job-state").unwrap_or(0);
            by_printer.entry(printer).or_default().push(json!({
                "id": j.int("job-id").unwrap_or(0),
                "title": if title.is_empty() { "Untitled document".to_string() } else { title },
                "owner": owner,
                "mine": owner == self.cups.user,
                "kb": j.int("job-k-octets").unwrap_or(0),
                "state": job_state_words(state),
                "printing": state == 5,
            }));
        }
        let mut list: Vec<Value> = printers
            .iter()
            .map(|p| {
                let name = p.text("printer-name");
                let reasons = p.strs("printer-state-reasons");
                let (state, status) = state_words(
                    p.int("printer-state").unwrap_or(3),
                    &reasons,
                    &p.text("printer-state-message"),
                );
                let mut problems: Vec<&str> =
                    reasons.iter().filter_map(|r| reason_words(r)).collect();
                problems.dedup();
                let uri = p.text("device-uri");
                let info = p.text("printer-info");
                // CUPS's own "Paused." or "Printing." says no more than the status does.
                let message = p.text("printer-state-message");
                let said = message.trim().trim_end_matches('.').to_lowercase();
                let message = if ["", "paused", "idle", "ready", "printing", "stopped"]
                    .contains(&said.as_str())
                    || said == status.to_lowercase()
                {
                    String::new()
                } else {
                    message
                };
                json!({
                    "name": name,
                    "info": if info.is_empty() { name.clone() } else { info },
                    "location": p.text("printer-location"),
                    "model": p.text("printer-make-and-model"),
                    "uri": uri,
                    "connection": connection_words(&uri),
                    "state": state,
                    "status": status,
                    "message": message,
                    "problems": problems,
                    "accepting": p.bool("printer-is-accepting-jobs").unwrap_or(true),
                    "default": default.as_deref() == Some(name.as_str()),
                    // printer-type bit 0: a class (a group of printers), not one printer.
                    "group": p.int("printer-type").unwrap_or(0) & 1 == 1,
                    "jobs": by_printer.remove(&name).unwrap_or_default(),
                })
            })
            .collect();
        list.sort_by_key(|p| {
            (
                p["default"] != true,
                p["info"].as_str().unwrap_or("").to_lowercase(),
            )
        });
        let added: Vec<String> = list
            .iter()
            .filter_map(|p| p["uri"].as_str().map(str::to_string))
            .collect();
        let found = self.shared.found.lock().unwrap().clone().map(|f| {
            f.into_iter()
                .map(|mut d| {
                    let uri = d["uri"].as_str().unwrap_or("").to_string();
                    d["added"] = json!(added.contains(&uri));
                    d
                })
                .collect::<Vec<_>>()
        });
        Ok(json!({
            "available": true,
            "printers": list,
            "default": default,
            "found": found,
        }))
    }
}
