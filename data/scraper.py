import json

import requests
from bs4 import BeautifulSoup

output_file = "element_data.json"

url1 = (
    "https://pubchem.ncbi.nlm.nih.gov/rest/pug/periodictable/JSON?response_type=display"
)
url2 = "https://chem.libretexts.org/Ancillary_Materials/Reference/Reference_Tables/Atomic_and_Molecular_Properties/A3%3A_Covalent_Radii"

headers = {  # Pretend this script is a browser
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:106.0) Gecko/20100101 Firefox/106.0",
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
    "Accept-Language": "en-US,en;q=0.5",
}

names = {
    "Uut": "Nh",
    "Uuq": "Fl",
    "Uup": "Mc",
    "Uuh": "Lv",
    "Uus": "Ts",
    "Uuo": "Og",
    "v": "V",
}

# 1. Extract data from PubChem
response1 = requests.get(url1).json()
data = {}

for row in response1["Table"]["Row"]:
    element, hex, radius_str = row["Cell"][1], row["Cell"][4], row["Cell"][7]

    color_rgb = [-1, -1, -1]
    if len(hex) == 6:
        color_rgb = [int(hex[0:2], 16), int(hex[2:4], 16), int(hex[4:6], 16)]

    van_der_wall = -1
    if len(radius_str) > 0:
        van_der_wall = int(radius_str)

    data[element] = {"color": color_rgb, "van_der_waal": van_der_wall}

# 2. Extract covalent radii from Chem Libretexts
response2 = requests.get(url2, headers=headers)
soup = BeautifulSoup(response2.content, "html.parser")

table_selector = "#elm-main-content > section > table > tbody"
table_rows = soup.select_one(table_selector).find_all("tr", recusive=False)

for row in table_rows:
    cells = [cell.get_text() for cell in row.find_all("td", recursive=False)]
    single_bond, double_bond, triple_bond = cells[3], cells[4], cells[5]
    element = cells[1]
    if element in names:
        element = names[element]
    data[element]["covalent"] = {
        "single_bond": -1 if single_bond == "-" else int(single_bond),
        "double_bond": -1 if double_bond == "-" else int(double_bond),
        "triple_bond": -1 if triple_bond == "-" else int(triple_bond),
    }

with open(output_file, "w") as file:
    file.write(json.dumps(data, indent=2))
