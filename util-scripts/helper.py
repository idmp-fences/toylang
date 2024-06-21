import csv, sys

with open(sys.argv[1], 'r') as f:
    r = csv.DictReader(f)
    for row in r:
        if row['file_name'] == sys.argv[2]:
            print(row['ilp-min-fences'])

