import re as _re
import threading
import time


class NotebookAccessError(Exception):
  def __init__(self, key):
    super().__init__(f'Notebook does not have access to secret {key}')


class SecretNotFoundError(Exception):
  def __init__(self, key):
    super().__init__(f'Secret {key} does not exist.')


class TimeoutException(Exception):
  def __init__(self, key):
    super().__init__(f'Requesting secret {key} timed out. Secrets can only be fetched when running from the Colab Notebook app.')


_lock = threading.Lock()
_msg_id = 0


def _request(request_type, request, timeout_sec):
  import zmq
  global _msg_id
  kernel = get_ipython().kernel
  _msg_id += 1
  parent = kernel.get_parent('shell') or {}
  route = {'header': {'session': parent['header']['session']}} if parent.get('header') else None
  msg = kernel.session.msg('colab_request', content={'request': request}, metadata={'colab_request_type': request_type, 'colab_msg_id': _msg_id}, parent=route)
  kernel.session.send(kernel.iopub_socket, msg)
  deadline = time.time() + timeout_sec
  while time.time() < deadline:
    try:
      _, reply = kernel.session.recv(kernel.stdin_socket, zmq.NOBLOCK)
    except Exception:
      reply = None
    value = reply.get('content', {}).get('value') if reply else None
    if isinstance(value, dict) and value.get('type') == 'colab_reply' and value.get('colab_msg_id') == _msg_id:
      if 'error' in value:
        raise RuntimeError(value['error'])
      return value.get('data')
    if reply is None:
      time.sleep(0.025)
  return None


def get(key):
  if not key or not isinstance(key, str):
    raise ValueError('Please enter a valid secret name')
  if _re.search(r'\s', key):
    raise ValueError('Secret name cannot contain spaces or whitespace')
  with _lock:
    resp = _request('GetSecret', {'key': key}, 10)
  if not resp:
    raise TimeoutException(key)
  if not resp.get('exists', False):
    raise SecretNotFoundError(key)
  if not resp.get('access', False):
    raise NotebookAccessError(key)
  return resp.get('payload', '')
