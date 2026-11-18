// Keel's call recorder, loaded into the Video meeting page from
// /keel-recording/recorder.js. When anyone in the call presses Record, each
// person's browser records its own camera and microphone at full quality
// and sends it to Keel in pieces. The pieces wait in this browser's storage
// until Keel has them, so a slow connection only makes them arrive later.
// Keel keeps one file per person and puts everyone in one video afterwards.
(() => {
  if (window.keelRecorder) return;
  const BASE = "/keel-recording";
  const PIECE_MS = 4000;
  const DB_NAME = "keel-recording";

  // ---- the pieces waiting for Keel, kept in this browser ----------------
  let dbPromise = null;
  const db = () => {
    if (!dbPromise) {
      dbPromise = new Promise((resolve, reject) => {
        const open = indexedDB.open(DB_NAME, 1);
        open.onupgradeneeded = () => {
          open.result.createObjectStore("pieces", { keyPath: ["take", "seq"] });
          open.result.createObjectStore("takes", { keyPath: "id" });
        };
        open.onsuccess = () => resolve(open.result);
        open.onerror = () => reject(open.error);
      });
    }
    return dbPromise;
  };
  const tx = async (store, mode, work) => {
    const base = await db();
    return new Promise((resolve, reject) => {
      const t = base.transaction(store, mode);
      const result = work(t.objectStore(store));
      t.oncomplete = () => resolve(result && "result" in result ? result.result : result);
      t.onerror = () => reject(t.error);
    });
  };
  const putPiece = (piece) => tx("pieces", "readwrite", (s) => s.put(piece));
  const dropPiece = (take, seq) => tx("pieces", "readwrite", (s) => s.delete([take, seq]));
  const firstPiece = async () => {
    const base = await db();
    return new Promise((resolve) => {
      const request = base.transaction("pieces", "readonly").objectStore("pieces").openCursor();
      request.onsuccess = () => resolve(request.result ? request.result.value : null);
      request.onerror = () => resolve(null);
    });
  };
  const putTake = (take) => tx("takes", "readwrite", (s) => s.put(take));
  const allTakes = () => tx("takes", "readonly", (s) => s.getAll());
  const dropTake = (id) => tx("takes", "readwrite", (s) => s.delete(id));
  const piecesOf = (take) =>
    tx("pieces", "readonly", (s) => s.count(IDBKeyRange.bound([take, 0], [take, Number.MAX_SAFE_INTEGER])));
  const waitingBytes = async () => {
    const base = await db();
    return new Promise((resolve) => {
      let bytes = 0;
      const cursor = base.transaction("pieces", "readonly").objectStore("pieces").openCursor();
      cursor.onsuccess = () => {
        const c = cursor.result;
        if (!c) return resolve(bytes);
        bytes += c.value.blob.size;
        c.continue();
      };
      cursor.onerror = () => resolve(bytes);
    });
  };

  // ---- sending, one piece at a time, again after a lost connection -----
  let sending = false;
  const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
  const send = async () => {
    if (sending) return;
    sending = true;
    try {
      for (;;) {
        const piece = await firstPiece().catch(() => null);
        if (!piece) break;
        const takes = await allTakes();
        const take = takes.find((t) => t.id === piece.take);
        if (!take) {
          await dropPiece(piece.take, piece.seq);
          continue;
        }
        let res;
        try {
          res = await fetch(`${BASE}/${take.id}/${piece.seq}`, {
            method: "PUT",
            headers: { "x-keel-token": take.token },
            body: piece.blob,
          });
        } catch {
          show();
          await sleep(3000);
          continue;
        }
        if (res.ok || res.status === 404) {
          await dropPiece(piece.take, piece.seq);
        } else {
          show(`Keel did not take the recording: ${(await res.json().catch(() => ({}))).message || res.status}`);
          await sleep(10000);
        }
        show();
      }
      // Recordings that stopped and have all their pieces at Keel are done.
      for (const take of await allTakes()) {
        if (!take.stopped || (await piecesOf(take.id)) > 0) continue;
        try {
          const res = await fetch(`${BASE}/${take.id}/done`, { method: "POST", headers: { "x-keel-token": take.token } });
          if (res.ok || res.status === 404) await dropTake(take.id);
        } catch { /* again on the next round */ }
      }
    } finally {
      sending = false;
      show();
    }
  };

  // ---- recording: the camera drawn on a canvas, and the microphone ------
  // A canvas keeps it one recording when Jitsi swaps the camera (turning it
  // off and on): a camera that is off is black, never recorded behind it.
  let rec = null;
  const meeting = () => {
    const app = window.APP;
    if (!app || !app.conference || !app.store) return null;
    const room = app.conference.roomName || (app.conference._room && app.conference._room.getName && app.conference._room.getName());
    if (!room) return null;
    const joined = typeof app.conference.isJoined === "function" ? app.conference.isJoined() : true;
    if (!joined) return null;
    const name = (app.conference.getLocalDisplayName && app.conference.getLocalDisplayName()) || "";
    return { room, name };
  };
  const localTrack = (kind) => {
    const tracks = (window.APP.store.getState()["features/base/tracks"] || []).filter((t) => t.local && t.mediaType === (kind === "audio" ? "audio" : "video"));
    const jt = tracks.length && tracks[0].jitsiTrack;
    const track = jt && (jt.getTrack ? jt.getTrack() : jt.track);
    return track && track.readyState === "live" && !tracks[0].muted ? track : null;
  };
  const pickType = () => {
    const kinds = [
      ["video/webm;codecs=vp8,opus", "webm"],
      ["video/webm", "webm"],
      ["video/mp4;codecs=avc1,mp4a", "mp4"],
      ["video/mp4", "mp4"],
    ];
    return kinds.find(([type]) => window.MediaRecorder && MediaRecorder.isTypeSupported(type)) || null;
  };

  const startRecording = async (where) => {
    const type = pickType();
    if (!type) {
      show("This browser cannot record. Chrome, Edge and Safari can.");
      return;
    }
    const res = await fetch(`${BASE}/start`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ room: where.room, name: where.name, kind: type[1] }),
    }).catch(() => null);
    const answer = res ? await res.json().catch(() => ({})) : {};
    if (!res || !res.ok || !answer.ok) {
      show(answer.message || "Keel could not start the recording.");
      return;
    }
    const take = { id: answer.id, token: answer.token, stopped: false };
    await putTake(take);

    const canvas = document.createElement("canvas");
    canvas.width = 1280;
    canvas.height = 720;
    const draw = canvas.getContext("2d");
    const video = document.createElement("video");
    video.muted = true;
    video.playsInline = true;
    let videoTrack = null;
    const audio = new AudioContext();
    const sink = audio.createMediaStreamDestination();
    let audioTrack = null;
    let audioSource = null;
    const follow = () => {
      const v = localTrack("video");
      if (v !== videoTrack) {
        videoTrack = v;
        video.srcObject = v ? new MediaStream([v]) : null;
        if (v) {
          const size = v.getSettings ? v.getSettings() : {};
          if (size.width && size.height && (size.width !== canvas.width || size.height !== canvas.height) && !rec.started) {
            canvas.width = size.width;
            canvas.height = size.height;
          }
          video.play().catch(() => {});
        }
      }
      const a = localTrack("audio");
      if (a !== audioTrack) {
        if (audioSource) audioSource.disconnect();
        audioSource = null;
        audioTrack = a;
        if (a) {
          audioSource = audio.createMediaStreamSource(new MediaStream([a]));
          audioSource.connect(sink);
        }
      }
    };
    const paint = () => {
      draw.fillStyle = "#000";
      draw.fillRect(0, 0, canvas.width, canvas.height);
      if (videoTrack && video.videoWidth) {
        const scale = Math.min(canvas.width / video.videoWidth, canvas.height / video.videoHeight);
        const w = video.videoWidth * scale;
        const h = video.videoHeight * scale;
        draw.drawImage(video, (canvas.width - w) / 2, (canvas.height - h) / 2, w, h);
      }
    };
    rec = { take, started: false, canvas, video, audio, recorder: null, timers: [] };
    follow();
    await new Promise((done) => setTimeout(done, 300));
    paint();
    const stream = new MediaStream([...canvas.captureStream(30).getVideoTracks(), ...sink.stream.getAudioTracks()]);
    const recorder = new MediaRecorder(stream, { mimeType: type[0], videoBitsPerSecond: 2500000, audioBitsPerSecond: 128000 });
    rec.recorder = recorder;
    // Numbered by this recording alone: the last piece arrives after Stop.
    let next = 0;
    recorder.ondataavailable = (event) => {
      if (!event.data || !event.data.size) return;
      const seq = next++;
      putPiece({ take: take.id, seq, blob: event.data }).then(send).catch(() => show("This browser has no room left for the recording."));
    };
    recorder.onstop = async () => {
      take.stopped = true;
      await putTake(take);
      audio.close().catch(() => {});
      send();
    };
    recorder.start(PIECE_MS);
    rec.started = true;
    rec.timers.push(setInterval(follow, 1000));
    rec.timers.push(setInterval(paint, 1000 / 30));
    show();
  };

  const stopRecording = () => {
    if (!rec) return;
    rec.timers.forEach(clearInterval);
    if (rec.recorder && rec.recorder.state !== "inactive") rec.recorder.stop();
    rec = null;
    show();
  };

  // ---- the call's recording state, kept by Keel for everyone ------------
  let state = { recording: false };
  const room = () => meeting();
  // One check at a time: a recording that is still starting is not started twice.
  let checking = false;
  const check = async () => {
    if (checking) return;
    checking = true;
    try {
      const where = room();
      if (!where) {
        if (rec) stopRecording();
        return;
      }
      const res = await fetch(`${BASE}/room/${encodeURIComponent(where.room)}`).catch(() => null);
      if (res && res.ok) state = await res.json().catch(() => state);
      if (state.recording && !rec) await startRecording(where).catch((err) => show(`Keel could not record: ${err.message || err}`));
      if (!state.recording && rec) stopRecording();
    } finally {
      checking = false;
      show();
    }
  };
  const toggle = async () => {
    const where = room();
    if (!where) return;
    button.disabled = true;
    const res = await fetch(`${BASE}/room/${encodeURIComponent(where.room)}/${state.recording ? "stop" : "start"}`, { method: "POST" }).catch(() => null);
    const answer = res ? await res.json().catch(() => ({})) : {};
    if (res && res.ok) state = { recording: Boolean(answer.recording) };
    else show(answer.message || "Keel did not answer. Try again.");
    button.disabled = false;
    while (checking) await sleep(100);
    await check();
  };

  // ---- what people see -----------------------------------------------------
  const box = document.createElement("div");
  box.id = "keel-recording";
  box.setAttribute("style", "position:fixed;left:12px;bottom:84px;z-index:10000;display:flex;flex-direction:column;gap:6px;align-items:flex-start;font:14px system-ui,sans-serif;");
  const button = document.createElement("button");
  button.type = "button";
  button.setAttribute("style", "border:0;border-radius:18px;padding:8px 14px;background:#2a2a2a;color:#fff;cursor:pointer;box-shadow:0 1px 4px rgba(0,0,0,.4);");
  button.addEventListener("click", toggle);
  const line = document.createElement("div");
  line.setAttribute("style", "max-width:320px;padding:6px 10px;border-radius:8px;background:rgba(0,0,0,.65);color:#fff;");
  line.hidden = true;
  box.append(button, line);
  let note = "";
  const show = async (message) => {
    if (message !== undefined) note = message;
    const inCall = Boolean(room());
    button.hidden = !inCall;
    button.textContent = state.recording ? "■ Stop recording" : "● Record call";
    button.style.background = state.recording ? "#b3261e" : "#2a2a2a";
    const left = await waitingBytes().catch(() => 0);
    const words = [];
    if (state.recording) words.push("This call is being recorded. Each person's video goes to Keel.");
    if (left > 0) words.push(`Sending your recording to Keel: ${(left / 1048576).toFixed(left > 10485760 ? 0 : 1)} MB left. Keep this page open.`);
    if (note) words.push(note);
    line.textContent = words.join(" ");
    line.hidden = !words.length;
    box.hidden = !inCall && left === 0 && !note;
  };
  window.addEventListener("beforeunload", (event) => {
    if (rec) stopRecording();
  });
  const mount = () => {
    if (!document.body) return setTimeout(mount, 200);
    document.body.appendChild(box);
    show();
  };
  mount();
  setInterval(check, 4000);
  setInterval(send, 15000);
  send();
  window.keelRecorder = { check, send, start: () => toggle(), state: () => state, recording: () => Boolean(rec) };
})();
