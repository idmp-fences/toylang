with open("alns-cbc.txt", 'r') as f:
    lines = f.readlines()

def merge_dicts(d1, d2):
    for (key, value) in d2.items():
        if key not in d1:
            d1[key] = value
        else:
            for i in range(len(d1[key])):
                d1[key][i] += d2[key][i]
    return d1

i = 0
destroy_ops = {}
repair_ops = {}
while i < len(lines):
    incumbents = []
    for _ in range(3):
        incumbents.append(lines[i].rstrip('\n'))
        destroy = eval(lines[i+1])
        repair = eval(lines[i+2])
        merge_dicts(destroy_ops, destroy)
        merge_dicts(repair_ops, repair)
        i += 3
    print(",".join(incumbents))

print(incumbents)
print(destroy_ops)
print(repair_ops)