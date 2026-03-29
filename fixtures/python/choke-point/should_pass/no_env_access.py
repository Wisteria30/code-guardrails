# Clean application code with no direct env access
class UserService:
    def __init__(self, config):
        self.timeout = config.timeout
