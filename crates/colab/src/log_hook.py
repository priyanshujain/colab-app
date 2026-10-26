def _colab_app_log(path):
    import os, threading
    session = get_ipython().kernel.session
    if getattr(session, "_colab_app_log", None) == path:
        return
    os.makedirs(os.path.dirname(path), exist_ok=True)
    log, lock, send = open(path, "ab"), threading.Lock(), session.send
    kinds = {"stream", "display_data", "update_display_data", "execute_input", "execute_result", "error", "status", "clear_output"}
    def logged(stream, msg_or_type, content=None, parent=None, ident=None, buffers=None, track=False, header=None, metadata=None):
        msg = session.msg(msg_or_type, content, parent, header, metadata) if isinstance(msg_or_type, str) else msg_or_type
        if msg["header"]["msg_type"] in kinds and log.tell() < 100 << 20:
            with lock:
                msg["metadata"]["colab_app_log"] = log.tell()
                log.write(session.pack({**msg, "channel": "iopub"}) + b"\n")
                log.flush()
        return send(stream, msg, ident=ident, buffers=buffers, track=track)
    session.send, session._colab_app_log = logged, path
