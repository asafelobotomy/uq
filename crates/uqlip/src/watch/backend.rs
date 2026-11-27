//! `backend!`: one data-control backend (ext or wlr) as a module, written once for both
//! protocols. Expanded in `watch.rs`, so its paths resolve there.

macro_rules! backend {
    ($m:ident, $client:path, $mgr:ident $Mgr:ident, $dev:ident $Dev:ident, $src:ident $Src:ident, $off:ident $Off:ident, $label:literal) => {
        mod $m {
            use super::*;
            use $client as p;
            use wayland_client::globals::{GlobalListContents, registry_queue_init};
            use wayland_client::protocol::wl_registry;
            use wayland_client::{Dispatch, Proxy, QueueHandle, event_created_child};

            struct State {
                events: Sender<Event>,
                policy: Policy,
                conn: Connection,
                max_bytes: u64,
                source: Option<p::$src::$Src>,
                pending: Option<(Vec<String>, Arc<Vec<u8>>)>,
            }

            type Offers = Mutex<Vec<String>>;

            impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
                fn event(
                    _: &mut Self,
                    _: &wl_registry::WlRegistry,
                    _: wl_registry::Event,
                    _: &GlobalListContents,
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                }
            }

            impl Dispatch<wayland_client::protocol::wl_seat::WlSeat, ()> for State {
                fn event(
                    _: &mut Self,
                    _: &wayland_client::protocol::wl_seat::WlSeat,
                    _: wayland_client::protocol::wl_seat::Event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                }
            }

            impl Dispatch<p::$mgr::$Mgr, ()> for State {
                fn event(
                    _: &mut Self,
                    _: &p::$mgr::$Mgr,
                    _: p::$mgr::Event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                }
            }

            impl Dispatch<p::$off::$Off, Offers> for State {
                fn event(
                    _: &mut Self,
                    _: &p::$off::$Off,
                    ev: p::$off::Event,
                    offers: &Offers,
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                    if let p::$off::Event::Offer { mime_type } = ev {
                        offers.lock().unwrap().push(mime_type);
                    }
                }
            }

            impl Dispatch<p::$src::$Src, ()> for State {
                fn event(
                    state: &mut Self,
                    src: &p::$src::$Src,
                    ev: p::$src::Event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                    match ev {
                        p::$src::Event::Send { mime_type: _, fd } => {
                            let data = state
                                .pending
                                .as_ref()
                                .map(|(_, d)| d.clone())
                                .unwrap_or_default();
                            std::thread::spawn(move || {
                                use std::io::Write;
                                let mut f = std::fs::File::from(fd);
                                let _ = f.write_all(&data);
                            });
                        }
                        p::$src::Event::Cancelled => {
                            src.destroy();
                            if state.source.as_ref() == Some(src) {
                                state.source = None;
                                state.pending = None;
                            }
                        }
                        _ => {}
                    }
                }
            }

            impl Dispatch<p::$dev::$Dev, ()> for State {
                fn event(
                    state: &mut Self,
                    _: &p::$dev::$Dev,
                    ev: p::$dev::Event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                    if let p::$dev::Event::Selection { id } = ev {
                        match id {
                            Some(offer) => state.capture(offer),
                            None => {
                                let _ = state.events.send(Event::Ignored);
                            }
                        }
                    }
                }

                event_created_child!(State, p::$dev::$Dev, [
                    p::$dev::EVT_DATA_OFFER_OPCODE => (p::$off::$Off, Mutex::new(Vec::new())),
                ]);
            }

            impl State {
                fn capture(&self, offer: p::$off::$Off) {
                    let offered: Vec<String> = offer
                        .data::<Offers>()
                        .map(|o| o.lock().unwrap().clone())
                        .unwrap_or_default();
                    let events = self.events.clone();
                    let policy = self.policy.clone();
                    let conn = self.conn.clone();
                    let max = self.max_bytes;
                    std::thread::spawn(move || {
                        let ask = |mime: &str, cap: u64| -> io::Result<Vec<u8>> {
                            let (r, w) = pipe()?;
                            offer.receive(mime.to_owned(), w.as_fd());
                            drop(w);
                            conn.flush().map_err(io::Error::other)?;
                            read_capped(r, cap, Duration::from_secs(2))
                        };
                        let hint = if offered.iter().any(|m| m == SECRET_HINT_MIME) {
                            ask(SECRET_HINT_MIME, 256)
                                .ok()
                                .map(|b| String::from_utf8_lossy(&b).into_owned())
                        } else {
                            None
                        };
                        let chosen = if policy(&offered, hint.as_deref()) {
                            crate::privacy::choose(&offered)
                        } else {
                            None
                        };
                        let ev = match chosen.and_then(|(mime, _)| {
                            ask(&mime, max).ok().map(|data| (mime, data))
                        }) {
                            Some((mime, data)) => Event::Copied(Captured { offered, mime, data }),
                            None => Event::Ignored,
                        };
                        offer.destroy();
                        let _ = conn.flush();
                        let _ = events.send(ev);
                    });
                }
            }

            pub fn available(conn: &Connection) -> bool {
                registry_queue_init::<State>(conn)
                    .map(|(g, _)| {
                        g.contents().with_list(|l| {
                            l.iter().any(|x| x.interface == p::$mgr::$Mgr::interface().name)
                        })
                    })
                    .unwrap_or(false)
            }

            pub fn run(
                conn: Connection,
                events: Sender<Event>,
                policy: Policy,
                max_bytes: u64,
                rx: Receiver<Request>,
                wake: OwnedFd,
            ) -> io::Result<()> {
                let (globals, mut queue) = registry_queue_init::<State>(&conn)
                    .map_err(io::Error::other)?;
                let qh = queue.handle();
                let mgr: p::$mgr::$Mgr = globals.bind(&qh, 1..=1, ()).map_err(io::Error::other)?;
                let seat = globals
                    .bind::<wayland_client::protocol::wl_seat::WlSeat, _, _>(&qh, 1..=1, ())
                    .map_err(io::Error::other)?;
                let dev = mgr.get_data_device(&seat, &qh, ());
                let mut state = State {
                    events,
                    policy,
                    conn: conn.clone(),
                    max_bytes,
                    source: None,
                    pending: None,
                };
                loop {
                    queue.dispatch_pending(&mut state).map_err(io::Error::other)?;
                    queue.flush().map_err(io::Error::other)?;
                    let Some(guard) = queue.prepare_read() else {
                        continue;
                    };
                    let mut fds = [
                        libc::pollfd { fd: guard.connection_fd().as_raw_fd(), events: libc::POLLIN, revents: 0 },
                        libc::pollfd { fd: wake.as_raw_fd(), events: libc::POLLIN, revents: 0 },
                    ];
                    let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
                    if n < 0 {
                        let e = io::Error::last_os_error();
                        if e.kind() == io::ErrorKind::Interrupted {
                            continue;
                        }
                        return Err(e);
                    }
                    if fds[0].revents != 0 {
                        guard.read().map_err(io::Error::other)?;
                    } else {
                        drop(guard);
                    }
                    if fds[1].revents != 0 {
                        let mut sink = [0u8; 64];
                        let _ = unsafe { libc::read(wake.as_raw_fd(), sink.as_mut_ptr().cast(), 64) };
                    }
                    while let Ok(req) = rx.try_recv() {
                        match req {
                            Request::Stop => return Ok(()),
                            Request::Set { mimes, data } => {
                                if let Some(old) = state.source.take() {
                                    old.destroy();
                                }
                                let src = mgr.create_data_source(&qh, ());
                                for m in &mimes {
                                    src.offer(m.clone());
                                }
                                dev.set_selection(Some(&src));
                                state.source = Some(src);
                                state.pending = Some((mimes, data));
                            }
                        }
                    }
                }
            }
        }
    };
}
