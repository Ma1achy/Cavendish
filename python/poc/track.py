"""Tracking proof of concept: recover an object's path from the gradiometer array's signals.

    cargo run --release -p generate --example tracking_dataset -- <data_dir>
    python python/poc/track.py <data_dir>

A small network is trained on **point masses only** and then asked to track bodies it has never
seen: a sphere, a cuboid, a walking articulated figure, and a point mass crossing a room whose air is
being heated by a radiator (convective gravity-gradient noise in the signal). The input is the
(T, D) phase signal with shot noise; the target is the object's horizontal position at each
measurement. The engine dumps the whole simulation state — choosing these two arrays as input and
target is this script's decision, not the engine's.
"""

import sys
from pathlib import Path

import numpy as np
import torch
from torch import nn

SCALE_M = 5.0  # positions are learned in units of 5 m


def load(directory: Path):
    signal = np.load(directory / "signal.npy")  # (B, T, D)
    position = np.load(directory / "source_position.npy")[:, 0, :, :2]  # (B, T, 2)
    return signal, position


def features(signal: np.ndarray, s0: float) -> torch.Tensor:
    # asinh compresses the 1/r³ dynamic range while staying linear near zero.
    return torch.tensor(np.arcsinh(signal / s0), dtype=torch.float32).flatten(1)


def errors_m(model: nn.Module, x: torch.Tensor, y: torch.Tensor) -> torch.Tensor:
    """Horizontal distance error [m] at every measurement of every pass, shape (B, T)."""
    with torch.no_grad():
        err = (model(x) - y) * SCALE_M
    return err.reshape(len(x), -1, 2).norm(dim=-1)


def rmse_m(model: nn.Module, x: torch.Tensor, y: torch.Tensor) -> float:
    return float(errors_m(model, x, y).pow(2).mean().sqrt())


def main() -> None:
    data = Path(sys.argv[1] if len(sys.argv) > 1 else "tracking_dataset")
    torch.manual_seed(0)

    signal, position = load(data / "train_point")
    s0 = float(np.median(np.abs(signal)))
    n_val = len(signal) // 10
    x, y = features(signal, s0), torch.tensor(position / SCALE_M, dtype=torch.float32).flatten(1)
    (x_val, y_val), (x_train, y_train) = (x[:n_val], y[:n_val]), (x[n_val:], y[n_val:])

    model = nn.Sequential(
        nn.Linear(x.shape[1], 512), nn.GELU(),
        nn.Linear(512, 512), nn.GELU(),
        nn.Linear(512, 512), nn.GELU(),
        nn.Linear(512, y.shape[1]),
    )
    epochs = int(sys.argv[2]) if len(sys.argv) > 2 else 150
    optimiser = torch.optim.AdamW(model.parameters(), lr=1e-3, weight_decay=1e-4)
    schedule = torch.optim.lr_scheduler.OneCycleLR(
        optimiser, max_lr=2e-3, total_steps=epochs * ((len(x_train) + 255) // 256)
    )
    for epoch in range(epochs):
        order = torch.randperm(len(x_train))
        for start in range(0, len(order), 256):
            batch = order[start:start + 256]
            loss = nn.functional.mse_loss(model(x_train[batch]), y_train[batch])
            optimiser.zero_grad()
            loss.backward()
            optimiser.step()
            schedule.step()
        if (epoch + 1) % 50 == 0:
            print(f"epoch {epoch + 1:4d}  train {rmse_m(model, x_train, y_train):.3f} m"
                  f"  val {rmse_m(model, x_val, y_val):.3f} m", flush=True)

    # The floor to beat: always guessing the mean path of the training set.
    guess = (y_val - y_train.mean(0)) * SCALE_M
    baseline = float(guess.reshape(len(y_val), -1, 2).norm(dim=-1).pow(2).mean().sqrt())
    print(f"\nHorizontal position error (trained on {len(x_train)} point-mass passes;"
          f" predict-the-mean baseline {baseline:.2f} m RMS)")
    print(f"  {'set':22s} {'median':>8s} {'90th pct':>9s} {'RMS':>7s}")
    for name in ["test_point", "test_sphere", "test_cuboid", "test_walker", "test_point_radiator"]:
        if not (data / name).exists():
            continue
        signal, position = load(data / name)
        err = errors_m(model, features(signal, s0),
                       torch.tensor(position / SCALE_M, dtype=torch.float32).flatten(1))
        print(f"  {name:22s} {float(err.median()):7.3f}m {float(err.quantile(0.9)):8.3f}m"
              f" {float(err.pow(2).mean().sqrt()):6.3f}m   ({len(signal)} passes)")


if __name__ == "__main__":
    main()
