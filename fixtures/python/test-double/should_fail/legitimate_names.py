# Words containing mock/stub/fake as substrings should NOT trigger
# unless they are standalone identifiers matching the regex

def process_hammock_data(data):
    """Hammock contains 'mock' but is not a test double."""
    return data

class Stockbroker:
    """Stockbroker contains 'stub' but is not a test double."""
    pass
