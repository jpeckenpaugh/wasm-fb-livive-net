# Simple sliding-window rate limiter and request summary helper.
from dataclasses import dataclass
from typing import Dict, List


@dataclass
class Bucket:
    limit: int
    timestamps: List[float]

    def allow(self, now: float, window_sec: float = 60.0) -> bool:
        cutoff = now - window_sec
        self.timestamps = [t for t in self.timestamps if t > cutoff]
        if len(self.timestamps) >= self.limit:
            return False
        self.timestamps.append(now)
        return True


def summarize_usage(buckets: Dict[str, Bucket]) -> Dict[str, int]:
    """Return active request counts keyed by client identifier."""
    return {client: len(bucket.timestamps) for client, bucket in buckets.items()}


if __name__ == "__main__":
    clients = {"api-client-a": Bucket(limit=5, timestamps=[10.0, 12.5, 19.0])}
    allowed = clients["api-client-a"].allow(now=25.0)
    print("allowed:", allowed, summarize_usage(clients))
