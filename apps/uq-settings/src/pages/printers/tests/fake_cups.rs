//! A fake CUPS: printers, jobs and the IPP replies the page reads.

use super::*;

#[derive(Clone, Default)]
pub(crate) struct Printer {
    pub(crate) name: String,
    pub(crate) info: String,
    pub(crate) location: String,
    pub(crate) uri: String,
    pub(crate) driver: String,
    pub(crate) state: i32,
    pub(crate) reasons: Vec<String>,
    pub(crate) accepting: bool,
    pub(crate) ppd: Option<Vec<u8>>,
}

#[derive(Default)]
pub(crate) struct World {
    pub(crate) printers: Vec<Printer>,
    pub(crate) default: Option<String>,
    /// (id, printer, title, owner, state)
    pub(crate) jobs: Vec<(i32, String, String, String, i32)>,
    /// (uri, info, make and model, device id)
    pub(crate) devices: Vec<(String, String, String, String)>,
    /// (ppd-name, make and model)
    pub(crate) ppds: Vec<(String, String)>,
    /// Whether the person is one of CUPS's administrators.
    pub(crate) admin: bool,
    pub(crate) calls: Vec<(String, u16)>,
    /// (printer, job name, format, bytes)
    pub(crate) printed: Vec<(String, String, String, usize)>,
    /// The last driver file given back with a change.
    pub(crate) uploaded: Option<Vec<u8>>,
    /// The printer group of the request being handled.
    pub(crate) last_printer_group: Option<Group>,
}

#[derive(Default)]
pub(crate) struct FakeCups(pub(crate) StdMutex<World>);

pub(crate) fn name_of(uri: &str) -> String {
    let last = uri.rsplit('/').next().unwrap_or("");
    percent_encoding::percent_decode_str(last)
        .decode_utf8_lossy()
        .into_owned()
}

impl World {
    pub(crate) fn printer_group(&self, p: &Printer) -> (u8, ipp::Attrs) {
        let mut reasons: Vec<Out> = p
            .reasons
            .iter()
            .map(|r| Out::Str(tag::KEYWORD, r.clone()))
            .collect();
        if reasons.is_empty() {
            reasons.push(Out::Str(tag::KEYWORD, "none".into()));
        }
        (
            tag::PRINTER,
            vec![
                ("printer-name".into(), s(tag::NAME, &p.name)),
                ("printer-info".into(), s(tag::TEXT, &p.info)),
                ("printer-location".into(), s(tag::TEXT, &p.location)),
                (
                    "printer-make-and-model".into(),
                    s(tag::TEXT, "Example Laser"),
                ),
                ("device-uri".into(), s(tag::URI, &p.uri)),
                ("printer-state".into(), vec![Out::Enum(p.state)]),
                ("printer-state-reasons".into(), reasons),
                (
                    "printer-is-accepting-jobs".into(),
                    vec![Out::Bool(p.accepting)],
                ),
            ],
        )
    }

    pub(crate) fn handle(&mut self, path: &str, req: &Group, op_id: u16, data: Vec<u8>) -> Vec<u8> {
        self.calls.push((path.to_string(), op_id));
        let target = name_of(&req.text("printer-uri"));
        let find = |w: &World| w.printers.iter().position(|p| p.name == target);
        match op_id {
            op::CUPS_GET_PRINTERS => {
                let groups = self
                    .printers
                    .iter()
                    .map(|p| self.printer_group(p))
                    .collect();
                answer(0, groups)
            }
            op::CUPS_GET_DEFAULT => match &self.default {
                Some(d) => answer(
                    0,
                    vec![(tag::PRINTER, vec![("printer-name".into(), s(tag::NAME, d))])],
                ),
                None => not_found("no default destination"),
            },
            op::GET_JOBS => {
                let groups = self
                    .jobs
                    .iter()
                    .map(|(id, p, title, owner, st)| {
                        (
                            tag::JOB,
                            vec![
                                ("job-id".into(), vec![Out::Int(*id)]),
                                ("job-name".into(), s(tag::NAME, title)),
                                ("job-state".into(), vec![Out::Enum(*st)]),
                                ("job-originating-user-name".into(), s(tag::NAME, owner)),
                                (
                                    "job-printer-uri".into(),
                                    s(tag::URI, &format!("ipp://localhost/printers/{p}")),
                                ),
                                ("job-k-octets".into(), vec![Out::Int(12)]),
                            ],
                        )
                    })
                    .collect();
                answer(0, groups)
            }
            op::CUPS_ADD_MODIFY_PRINTER => {
                let attrs = |name: &str| {
                    // The printer group's attributes come after the operation group.
                    self.last_printer_group
                        .as_ref()
                        .and_then(|g| g.str(name).map(str::to_string))
                };
                let info = attrs("printer-info");
                let location = attrs("printer-location");
                let uri = attrs("device-uri");
                let driver = attrs("ppd-name");
                let i = match find(self) {
                    Some(i) => i,
                    None => {
                        self.printers.push(Printer {
                            name: target.clone(),
                            state: 3,
                            accepting: true,
                            ppd: Some(PPD.as_bytes().to_vec()),
                            ..Default::default()
                        });
                        self.printers.len() - 1
                    }
                };
                let p = &mut self.printers[i];
                if let Some(v) = info {
                    p.info = v;
                }
                if let Some(v) = location {
                    p.location = v;
                }
                if let Some(v) = uri {
                    p.uri = v;
                }
                if let Some(v) = driver {
                    p.driver = v;
                }
                if !data.is_empty() {
                    p.ppd = Some(data.clone());
                    self.uploaded = Some(data);
                }
                answer(0, vec![])
            }
            op::CUPS_DELETE_PRINTER
            | op::CUPS_SET_DEFAULT
            | op::PAUSE_PRINTER
            | op::RESUME_PRINTER
            | op::CUPS_ACCEPT_JOBS
            | op::PRINT_JOB => {
                let Some(i) = find(self) else {
                    return not_found(&format!("printer \"{target}\" does not exist"));
                };
                match op_id {
                    op::CUPS_DELETE_PRINTER => {
                        self.printers.remove(i);
                        if self.default.as_deref() == Some(target.as_str()) {
                            self.default = None;
                        }
                    }
                    op::CUPS_SET_DEFAULT => self.default = Some(target),
                    op::PAUSE_PRINTER => {
                        self.printers[i].state = 5;
                        self.printers[i].reasons = vec!["paused".into()];
                    }
                    op::RESUME_PRINTER => {
                        self.printers[i].state = 3;
                        self.printers[i].reasons.clear();
                    }
                    op::CUPS_ACCEPT_JOBS => self.printers[i].accepting = true,
                    _ => self.printed.push((
                        target,
                        req.text("job-name"),
                        req.text("document-format"),
                        data.len(),
                    )),
                }
                answer(0, vec![])
            }
            op::CANCEL_JOB => {
                let id: i32 = name_of(&req.text("job-uri")).parse().unwrap_or(-1);
                self.jobs.retain(|j| j.0 != id);
                answer(0, vec![])
            }
            op::CUPS_GET_DEVICES => {
                let groups = self
                    .devices
                    .iter()
                    .map(|(uri, info, mm, id)| {
                        (
                            tag::PRINTER,
                            vec![
                                ("device-class".into(), s(tag::KEYWORD, "network")),
                                ("device-info".into(), s(tag::TEXT, info)),
                                ("device-make-and-model".into(), s(tag::TEXT, mm)),
                                ("device-uri".into(), s(tag::URI, uri)),
                                ("device-id".into(), s(tag::TEXT, id)),
                            ],
                        )
                    })
                    .collect();
                answer(0, groups)
            }
            op::CUPS_GET_PPDS => {
                let groups = self
                    .ppds
                    .iter()
                    .map(|(n, mm)| {
                        (
                            tag::PRINTER,
                            vec![
                                ("ppd-name".into(), s(tag::NAME, n)),
                                ("ppd-make-and-model".into(), s(tag::TEXT, mm)),
                                ("ppd-natural-language".into(), s(tag::LANGUAGE, "en")),
                            ],
                        )
                    })
                    .collect();
                answer(0, groups)
            }
            other => panic!("the fake doesn't know operation {other:#06x}"),
        }
    }
}

impl Transport for FakeCups {
    fn ipp(
        &self,
        path: &'static str,
        body: Vec<u8>,
        _timeout: Duration,
    ) -> BoxFuture<'static, Result<Vec<u8>, CupsError>> {
        let (req, data) = ipp::decode_request(&body).expect("a well-formed request");
        let mut w = self.0.lock().unwrap();
        let out = if path == "/admin/" && !w.admin {
            w.calls.push((path.to_string(), req.status));
            Err(CupsError::NotAllowed)
        } else {
            let opg = req.groups(tag::OPERATION).next().cloned().unwrap();
            assert_eq!(opg.str("requesting-user-name"), Some("me"));
            w.last_printer_group = req.groups(tag::PRINTER).next().cloned();
            Ok(w.handle(path, &opg, req.status, data))
        };
        async move { out }.boxed()
    }

    fn get(&self, path: String) -> BoxFuture<'static, Result<Vec<u8>, CupsError>> {
        let w = self.0.lock().unwrap();
        let name = path
            .strip_prefix("/printers/")
            .and_then(|p| p.strip_suffix(".ppd"))
            .map(name_of)
            .unwrap_or_default();
        let out = w
            .printers
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.ppd.clone())
            .ok_or_else(|| CupsError::Failed("no driver file".into()));
        async move { out }.boxed()
    }
}

pub(crate) const PPD: &str = "*PPD-Adobe: \"4.3\"\n\
*OpenUI *PageSize/Media Size: PickOne\n\
*DefaultPageSize: Letter\n\
*PageSize Letter/US Letter: \"\"\n\
*PageSize A4/A4: \"\"\n\
*CloseUI: *PageSize\n\
*OpenUI *Duplex/2-Sided Printing: PickOne\n\
*DefaultDuplex: None\n\
*Duplex None/Off: \"\"\n\
*Duplex DuplexNoTumble/Long edge: \"\"\n\
*CloseUI: *Duplex\n";
