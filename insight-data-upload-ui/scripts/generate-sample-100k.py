#!/usr/bin/env python3
"""Generate the 100,000-row CSV used to exercise Data Uploads Utility import/pagination."""
import csv
from pathlib import Path

HEADERS = [
    "PAN","Name","DOB/DOI","Mobile","E-Mail","PIN Code","Address","State",
    "FY","Information Type","Findings","Source","Information Value","Description",
    "Actionable AY","Verification Result Type","Statutory Reason",
    "Income Escaping Assessment Value","Verification Information Value"
]
STATES = [
    ("Maharashtra","400001","Bandra West, Mumbai"),
    ("Karnataka","560001","Koramangala, Bengaluru"),
    ("Delhi","110001","Connaught Place, Delhi"),
    ("West Bengal","700001","Salt Lake, Kolkata"),
    ("Uttar Pradesh","226001","Gomti Nagar, Lucknow"),
    ("Punjab","160001","Sector 17, Chandigarh"),
]
INFO_TYPES = ["Financial Information","Property Information","Banking Information","Transaction Information"]
FINDINGS = ["Confirmed","Partially Confirmed","Not Confirmed","No Evidence"]
SOURCES = ["Digital","Portal","Third Party","Physical"]
VERIFICATIONS = ["Income Escaping Assessment","Further Verification Required","No Income Escaping"]
REASONS = ["Mismatch","Undisclosed Income","Unverified Information","Other"]

def main() -> None:
    root = Path(__file__).resolve().parents[1]
    output = root / "samples" / "sample-data-100k.csv"
    output.parent.mkdir(exist_ok=True)
    with output.open("w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, quoting=csv.QUOTE_MINIMAL, lineterminator="\r\n")
        writer.writerow(HEADERS)
        for i in range(1, 100_001):
            state, pin, address = STATES[(i - 1) % len(STATES)]
            pan = f"ABCDE{i % 10000:04d}{chr(65 + ((i // 10000) % 26))}"
            dob = f"{(i % 28) + 1:02d}/{((i // 28) % 12) + 1:02d}/{1980 + (i % 35):04d}"
            mobile = f"{9000000000 + i % 100000000:010d}"
            email = f"user{i}@example.in"
            fy = ["2025-26", "2024-25", "2023-24"][i % 3]
            info_type = INFO_TYPES[i % len(INFO_TYPES)]
            finding = FINDINGS[i % len(FINDINGS)]
            source = SOURCES[i % len(SOURCES)]
            value = str(10_000 + (i % 990_000))
            ay = ["2026-27", "2025-26", "2024-25"][i % 3]
            verification = VERIFICATIONS[i % len(VERIFICATIONS)]
            reason = REASONS[i % len(REASONS)]
            assessment = value if verification != "No Income Escaping" else "0"
            writer.writerow([
                pan, f"Sample Person {i}", dob, mobile, email, pin, address, state,
                fy, info_type, finding, source, value, f"Imported sample record {i}",
                ay, verification, reason, assessment, value
            ])
    print(f"Generated {output}")

if __name__ == "__main__":
    main()
