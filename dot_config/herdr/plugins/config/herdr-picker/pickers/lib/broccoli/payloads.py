import base64
import binascii
import json


def decoded(value):
    """A Temporal value with its payloads and the JSON stored in strings turned into plain JSON."""
    if isinstance(value, dict):
        if set(value) == {"payloads"}:
            items = [decoded(item) for item in value["payloads"] or []]
            return items[0] if len(items) == 1 else items
        if isinstance(value.get("metadata"), dict) and set(value) <= {"metadata", "data"}:
            return decoded(payload_data(value))
        return {key: decoded(item) for key, item in value.items()}
    if isinstance(value, list):
        return [decoded(item) for item in value]
    if isinstance(value, str) and value.lstrip()[:1] in ("{", "["):
        try:
            return decoded(json.loads(value))
        except ValueError:
            return value
    return value


def payload_data(payload):
    if unbase64(payload["metadata"].get("encoding", "")) == "binary/null":
        return None
    text = unbase64(payload.get("data", ""))
    try:
        return json.loads(text)
    except ValueError:
        return text


def unbase64(text):
    try:
        return base64.b64decode(text, validate=True).decode()
    except (binascii.Error, UnicodeDecodeError, ValueError):
        return text


def found(value, *keys):
    """The first value anywhere under one of these keys, compared without case."""
    wanted = {key.lower() for key in keys}
    if isinstance(value, dict):
        for key, item in value.items():
            if key.lower() in wanted:
                return item
        value = list(value.values())
    if isinstance(value, list):
        for item in value:
            hit = found(item, *keys)
            if hit is not None:
                return hit
    return None


def readable(value):
    """Pretty JSON with the line breaks inside strings shown as real ones, for previews."""
    if isinstance(value, str):
        return value
    return json.dumps(value, indent=2, ensure_ascii=False).replace("\\n", "\n")
