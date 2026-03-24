name = get_user_name()
config = load_config()
items = list(range(10))

if name and config:
    process(name, config)
