import contextlib

with contextlib.suppress(FileNotFoundError):
    data = open("config.json").read()
