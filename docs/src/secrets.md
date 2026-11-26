# Secrets

Secrets are name and value pairs, such as an S3 key or a Hugging Face token. Every notebook gets every secret. Code reads them as environment variables or with `google.colab.userdata.get`.

## Add a secret

Open **Settings > Secrets** and press **Add new secret**. Type a name and a value. The app saves the pair when a field loses focus. A name cannot be empty or contain whitespace.

Values stay hidden. Press **Show** to see one, and **Delete** to remove it.

## Read a secret

Both forms work on a local kernel, a Colab runtime and a remote kernel:

```python
import os
token = os.environ["HF_TOKEN"]

from google.colab import userdata
token = userdata.get("HF_TOKEN")
```

The server sets each secret in every kernel that it connects. It sets them again after a restart. When you change a secret in Settings, running kernels get the new value at once. A remote kernel gets the secrets of the machine that holds the notebook, not those of the kernel host.

## Where secrets live

On macOS all values live in one item in the login Keychain. The item has service `colab-app` and account `secrets`, next to the Google sign in. The signed app reads it without a password prompt. A build with another signature, such as `just dev`, makes macOS ask for your password.

On Linux the server keeps secrets in a `0600` file in its data folder.

## How userdata.get works

Notebooks written for Colab call `google.colab.userdata.get(name)`. That function sends Colab's `GetSecret` request, and the app answers it. The app uses the same protocol as the Colab web page.

<div class="diagram-frame"><svg class="diagram" viewbox="0 0 760 250" role="img" aria-labelledby="secret-title">
<title id="secret-title">The GetSecret request between the kernel and the server</title>
<defs><marker id="secret-arrow" viewbox="0 0 10 10" refx="9" refy="5" markerwidth="7" markerheight="7" orient="auto-start-reverse"><path class="head" d="M0 0L10 5L0 10z"/></marker></defs>
<rect class="box" x="40" y="16" width="180" height="40" rx="8"/>
<text class="title" x="56" y="41">Kernel</text>
<rect class="hot" x="300" y="16" width="180" height="40" rx="8"/>
<text class="title" x="316" y="41">colab-server</text>
<rect class="box" x="560" y="16" width="160" height="40" rx="8"/>
<text class="title" x="576" y="41">Keychain</text>
<path class="wire dash" d="M130 56V236"/>
<path class="wire dash" d="M390 56V236"/>
<path class="wire dash" d="M640 56V236"/>
<path class="wire" d="M130 96H386" marker-end="url(#secret-arrow)"/>
<text class="small" x="142" y="88">colab_request on iopub: GetSecret, key</text>
<path class="wire" d="M390 132H636" marker-end="url(#secret-arrow)"/>
<text class="small" x="402" y="124">read the secrets item</text>
<path class="wire" d="M640 160H394" marker-end="url(#secret-arrow)"/>
<text class="small" x="452" y="152">name to value map</text>
<path class="wire" d="M390 204H134" marker-end="url(#secret-arrow)"/>
<text class="small" x="142" y="196">input_reply on stdin: colab_reply with the value</text>
</svg></div>

*The kernel waits on its stdin socket. The server answers with an `input_reply`, as the Colab web page does.*

A Colab runtime has its own `google.colab` package. A local kernel does not, so the server puts a small `google.colab.userdata` module on its `PYTHONPATH`. That module speaks the same protocol. A missing name raises the same `SecretNotFoundError` as on Colab.

The server answers only `GetSecret`. Every other Colab request gets an error reply that says the app does not support it.

## Variables for one notebook

To pass a value to one kernel without storage, an agent uses the MCP `set_env` tool. The value lasts until the kernel restarts. See [Tools](agents-mcp.md#tools).
