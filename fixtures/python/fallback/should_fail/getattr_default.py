class Config:
    pass

cfg = Config()
debug = getattr(cfg, "debug", False)
