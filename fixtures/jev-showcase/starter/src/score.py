def calculate_score(base_points, combo):
    """Return points after applying the positive combo multiplier."""
    if combo < 1:
        raise ValueError("combo must be positive")
    return base_points + combo
