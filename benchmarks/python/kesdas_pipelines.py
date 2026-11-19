#!/usr/bin/env python3
"""Pandas twins of the Kesdas benchmark pipelines.

Usage: kestrel_pipelines.py DATA.csv PIPELINE
Prints one checksum. The driver times the whole process and subtracts the
`load` baseline, so both sides include the same CSV read.
"""
import sys

import pandas as pd


def load(path: str) -> pd.DataFrame:
    return pd.read_csv(
        path,
        dtype={
            "id": "int64",
            "region": "string",
            "product": "string",
            "units": "int64",
            "price": "float64",
            "discount": "float64",
            "active": "bool",
        },
    )


def run(df: pd.DataFrame, name: str) -> float:
    if name == "load":
        return float(len(df) + df["units"].sum())

    if name == "filter_sort":
        enriched = df.assign(revenue=df["units"] * df["price"])
        top = enriched[(enriched["active"]) & (enriched["revenue"] > 20)].nlargest(
            100, "revenue"
        )
        return float(top["revenue"].sum())

    if name == "groupby_agg":
        grouped = df.groupby("region", observed=True).agg(
            units_sum=("units", "sum"),
            price_mean=("price", "mean"),
            units_mean=("units", "mean"),
        )
        return float(grouped.sum().sum())

    if name == "groupby_two_keys":
        grouped = df[["region", "product", "units", "price"]].groupby(
            ["region", "product"], observed=True
        ).sum()
        return float(grouped.sum().sum())

    if name == "pivot":
        table = df.pivot_table(
            index="region", columns="product", values="units", aggfunc="sum"
        )
        return float(table.sum().sum())

    if name == "merge":
        lookup = pd.DataFrame(
            {
                "region": [f"r{i}" for i in range(8)],
                "weight": [1.0, 0.9, 1.1, 1.2, 0.8, 1.05, 0.95, 1.15],
            }
        )
        merged = df.merge(lookup, on="region", how="left")
        weighted = merged.assign(weighted=merged["units"] * merged["weight"])[
            ["region", "weighted"]
        ]
        return float(weighted.groupby("region", observed=True).sum().sum().sum())

    if name == "missing_fill":
        clean = df.fillna(0.0)
        net = clean.assign(
            net=clean["units"] * clean["price"] * (1.0 - clean["discount"])
        )
        return float(net.groupby("active", observed=True).mean(numeric_only=True).sum().sum())

    if name == "value_counts":
        return float(df["region"].value_counts().sum())

    if name == "rolling":
        return float(df[["units", "price"]].rolling(50).mean().sum().sum())

    if name == "corr":
        return float(df[["units", "price", "discount"]].corr().sum().sum())

    if name == "melt":
        long = df[["id", "units", "price"]].melt(
            id_vars="id", value_vars=["units", "price"]
        )
        return float(long["value"].sum())

    raise SystemExit(f"unknown pipeline {name}")


def main() -> None:
    if len(sys.argv) < 3:
        raise SystemExit("usage: kestrel_pipelines.py DATA.csv PIPELINE [REPS]")
    frame = load(sys.argv[1])
    reps = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    total = 0.0
    for _ in range(reps):
        total += run(frame, sys.argv[2])
    print(total)


if __name__ == "__main__":
    main()
