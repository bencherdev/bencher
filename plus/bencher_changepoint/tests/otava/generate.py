# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.
#
# Generates `vectors.rs` beside this script: the inputs below and the
# changepoints that Apache Otava finds in them, with their statistics.
#
# Run it with the Otava commit the port pins, and nothing else installed:
#
#   python3 -m venv venv
#   ./venv/bin/pip install git+https://github.com/apache/otava@d4f62a1c38dea6053ce31d1f1fba7e53105d2e66
#   ./venv/bin/python -I generate.py > vectors.rs
#
# Sources of the inputs:
# - Apache Otava at commit d4f62a1c38dea6053ce31d1f1fba7e53105d2e66,
#   Apache-2.0: the TigerBeetle series of `tests/tigerbeetle_test.py`, and the
#   series of `tests/analysis_test.py`, `tests/series_test.py`,
#   `tests/change_point_divisive_test.py`, and `examples/csv/data/local_sample.csv`.
# - MongoDB signal-processing-algorithms (commit
#   e00eaf60dba9e22852ca39f5b46187b41eee73e5), Apache-2.0: the series of
#   `tests/data/real_series.json`, as an input only.
# - Bencher: the edge series, built below from a fixed noise pattern; the
#   small series that pin the ties, the odd window step, the first pass, and
#   the order of a sum; and a long series from Python's seeded `random()`.

import json
import math
import random
from importlib.metadata import distribution

from otava.series import AnalysisOptions, Metric, Series

OTAVA = "d4f62a1c38dea6053ce31d1f1fba7e53105d2e66"

TIGERBEETLE = [
    26705, 26475, 26641, 26806, 26835, 26911, 26564, 26812, 26874, 26682, 15672, 26745,
    26460, 26977, 26851, 23412, 23547, 23674, 23519, 23670, 23662, 23462, 23750, 23717,
    23524, 23588, 23687, 23793, 23937, 23715, 23570, 23730, 23690, 23699, 23670, 23860,
    23988, 23652, 23681, 23798, 23728, 23604, 23523, 23412, 23685, 23773, 23771, 23718,
    23409, 23739, 23674, 23597, 23682, 23680, 23711, 23660, 23990, 23938, 23742, 23703,
    23536, 24363, 24414, 24483, 24509, 24944, 24235, 24560, 24236, 24667, 24730, 28346,
    28437, 28436, 28057, 28217, 28456, 28427, 28398, 28250, 28331, 28222, 28726, 28578,
    28345, 28274, 28514, 28590, 28449, 28305, 28411, 28788, 28404, 28821, 28580, 27483,
    26805, 27487, 27124, 26898, 27295, 26951, 27312, 27660, 27154, 27050, 26989, 27193,
    27503, 27326, 27375, 27513, 27057, 27421, 27574, 27609, 27123, 27824, 27644, 27394,
    27836, 27949, 27702, 27457, 27272, 28207, 27802, 27516, 27586, 28005, 27768, 28543,
    28237, 27915, 28437, 28342, 27733, 28296, 28524, 28687, 28258, 28611, 29360, 28590,
    29641, 28965, 29474, 29256, 28611, 28205, 28539, 27962, 28398, 28509, 28240, 28592,
    28102, 28461, 28578, 28669, 28507, 28535, 28226, 28536, 28561, 28087, 27953, 28398,
    28007, 28518, 28337, 28242, 28607, 28545, 28514, 28377, 28010, 28412, 28633, 28576,
    28195, 28637, 28724, 28466, 28287, 28719, 28425, 28860, 28842, 28604, 28327, 28216,
    28946, 28918, 29287, 28725, 29148, 29541, 29137, 29628, 29087, 28612, 29154, 29108,
    28884, 29234, 28695, 28969, 28809, 28695, 28634, 28916, 29852, 29389, 29757, 29531,
    29363, 29251, 29552, 29561, 29046, 29795, 29022, 29395, 28921, 29739, 29257, 29455,
    29376, 29528, 28909, 29492, 28984, 29621, 29026, 29457, 29102, 29114, 28924, 29162,
    29259, 29554, 29616, 29211, 29367, 29460, 28836, 29645, 29586, 28848, 29324, 28969,
    29150, 29243, 29081, 29312, 28923, 29272, 29117, 29072, 29529, 29737, 29652, 29612,
    29856, 29012, 30402, 29969, 29309, 29439, 29285, 29421, 29023, 28772, 29692, 29416,
    29267, 29542, 29904, 30045, 29739, 29945, 29141, 29163, 29765, 29197, 29441, 28910,
    29504, 29614, 29643, 29506, 29420, 29672, 29432, 29784, 29888, 29309, 29247, 29816,
    29254, 29813, 29451, 29382, 29618, 28558, 29845, 29499, 29283, 29184, 29246, 28790,
    29952, 29145, 29415, 30437, 29227, 29605, 29859, 29156, 29807, 29406, 29734, 29861,
    29140, 29983, 29832, 29919, 29896, 29991, 29266, 29001, 29459, 29548, 29310, 29042,
    29303, 29894, 29091, 29018, 29537, 29614, 29180, 29736, 29500, 29218, 29581, 28906,
    28542, 29306, 28987, 29878, 28865, 30272, 29707, 29662, 29815, 30492, 29347, 30096,
    29054, 30238, 28813, 31895, 28915,
]

MONGODB_REAL = [
    14969.153332576032, 15007.362736414036, 14966.075914634359, 14962.501499940001,
    14447.971204253925, 15009.655149518072, 14963.867028424698, 14996.61663161303,
    14995.825659877228, 14978.936433116012, 15013.424985823043, 14965.15826127094,
    14967.16539218737, 15008.256701107419, 15010.581564039561, 15014.16249298275,
    15008.140196370676, 15003.319371540321, 14967.272871266692, 15004.079358446263,
    14999.191032844066, 14976.956336484986, 15004.279351506504, 14962.674560203384,
    15012.177576220347, 14992.539279030963, 14996.657121012218, 14938.58602411441,
    14982.399205791848, 15006.338762930265, 15005.928274132753, 14962.949363463425,
    15001.356950878377, 14976.89193818731, 15007.932700143936, 14968.247671660916,
    14970.269751643456, 14951.749775608258, 14973.638488556679, 15016.903828917993,
    15001.517941103886, 15010.755109589521, 15003.399864005438, 15015.450434514098,
    15010.208233565103, 14969.178779080668, 14999.64351322566, 14960.073172934053,
    14999.883504787953, 15012.761041878597, 15012.7685479934, 15005.465268819129,
    15004.902804868365, 14998.236066323907, 15008.0131866844, 14998.485062112453,
    14976.217425142162, 15013.899444022238, 14971.15025207906, 14967.124982663281,
    14960.568664002378, 14979.782832947281, 15002.885401030744, 14987.342055683755,
    14953.540737576413, 14961.315012583007, 14975.348572337103, 15015.770839201836,
    14999.87650431, 14973.356145685735, 14970.398189992764, 15002.149421116244,
    15005.781761791413, 14997.039110737258, 14959.056760559297, 14999.193537984362,
    14965.052013247827, 15005.701810699884, 14957.582251852999, 15005.047814240435,
    15006.154735346381, 14965.87741949935, 15008.76663004821, 14965.977415339523,
    14996.063140939554, 14986.555988017635, 15019.838258049147, 15024.459119471698,
    15040.414927859307, 15022.918613049675, 13988.311092103024, 14961.01029742529,
    14967.286341260007, 15004.865294198056, 14971.113132365212, 15005.338800328867,
    14974.421987311289, 14986.041020669929, 15008.261203422799, 15009.077123299383,
    14977.515876880803, 14982.333667787358, 14980.438233566241, 15010.926582604545,
    15010.698071007353, 14975.368039468734, 15013.486538760375, 14978.440948598263,
    14976.297952822297, 14983.359100736463, 15006.756755405453, 14966.725000702469,
    15005.876294917307, 15017.01733972722, 15009.811615384677, 15008.103231783029,
    15005.990756775274, 15008.025192635123, 15007.141734327482, 14972.236729912864,
    14979.175368387137, 15006.883217099778, 15009.383114354001, 15014.512932544338,
    15004.28535301239, 15016.412858257241, 15029.17409089141, 14961.016281555932,
    14965.916057636165, 14964.020892391465, 14969.190460372178, 15005.072787957464,
]

SINGLE = [
    1.02, 0.95, 0.99, 1.00, 1.12, 1.00, 1.01, 0.98, 1.01, 0.96,
    0.50, 0.51, 0.48, 0.48, 0.55, 0.50, 0.49, 0.51, 0.50, 0.49,
]
SERIES_1 = [1.02, 0.95, 0.99, 1.00, 1.12, 0.90, 0.50, 0.51, 0.48, 0.48, 0.55]
SERIES_2 = [2.02, 2.03, 2.01, 2.04, 1.82, 1.85, 1.79, 1.81, 1.80, 1.76, 1.78]
DIV_BY_ZERO = [0.0, 0.0, 0.0, 1.00, 1.12, 0.90, 0.50, 0.51, 0.48, 0.48, 0.55]
PLATEAUS = [1] * 5 + [5] * 8 + [9] * 8 + [3] * 9
CSV_METRIC_1 = [154023, 138455, 143112, 149190, 132098, 151344, 155145, 148889, 149466, 148209]
CSV_METRIC_2 = [10.43, 10.23, 10.29, 10.91, 10.34, 10.69, 9.23, 9.11, 9.13, 9.03]

NOISE = [0.004, -0.007, 0.001, 0.006, -0.003, -0.005, 0.002, 0.007, -0.001, -0.006, 0.003, 0.005, -0.002, -0.004, 0.0, 0.006]


def noisy(level, length, phase=0):
    return [level * (1 + NOISE[(i + phase) % len(NOISE)]) for i in range(length)]


ONE_OUTLIER = noisy(100, 60)
ONE_OUTLIER[30] = 200.0
STEP_AT_LAST_POINT = noisy(100, 59) + [150.0]
TWO_STEPS_IN_ONE_WINDOW = noisy(100, 20) + noisy(110, 10, 3) + noisy(120, 20, 7)
THREE_POINTS = [10.0, 10.0, 20.0]
DOUBLING = [10.0] * 3 + [20.0] * 3
MERGE_NEIGHBORS = [1.0] * 4 + [2.0] * 4 + [2.05] * 4
MERGE_NEIGHBORS_TAIL = MERGE_NEIGHBORS + [3.0] * 4
# The small series below pin the faithful edges that the series above leave open.
# Each one came from a seeded search for the smallest series where the port with
# that edge changed gives other indices than Otava.
# At a p value of 1, the first pass keeps a candidate with a p value of exactly 1.
FIRST_PASS_AT_ONE = [1.0, 0.0, 0.0]
# The best split of an interval is the first maximum of Q in row major order.
Q_TIE = [4.0] * 5 + [5.0] * 10 + [6.0] * 5 + [4.0] * 5 + [3.0] * 2
# The best split over all intervals is the one in the first interval.
INTERVAL_TIE = [1.0, 1.0, 0.0, 6.0, 6.0, 7.0, 6.0, 7.0, 6.0, 7.0, 7.0, 6.0]
# An odd window steps by half a window rounded down.
ODD_WINDOW = [72.43, 72.38, 72.55, 72.61, 72.4, 86.7, 87.14, 86.95, 87.01, 86.73]
# The merge removes the first of two equal smallest magnitudes.
MAGNITUDE_TIE = [1.0] * 2 + [2.0] * 6 + [4.0] * 6
# A mean is numpy's pairwise sum, which decides a magnitude at the minimum.
PAIRWISE_SUM = [1.4] * 8 + [2.1] * 4
# Above 128 points, the pairwise sum adds two halves cut at a multiple of 8.
# A sum in another order moves a mean near 1e8 by an ulp, and so the t statistic.
_NEAR_CONSTANT_RNG = random.Random(0)
NEAR_CONSTANT = [
    1e8 + 0.2 * _NEAR_CONSTANT_RNG.random() - 0.1 + (0.2 if i >= 150 else 0.0)
    for i in range(300)
]
# Within an interval, Q subtracts the distances to the points before it, summed
# from the first point down as numpy sums them.
VERTICAL_SUM = [1.1, 1.1, 0.0, 0.3, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0]

DEFAULTS = (50, 0.001)
INPUTS = [
    ("TIGERBEETLE", TIGERBEETLE),
    ("MONGODB_REAL", MONGODB_REAL),
    ("SINGLE", SINGLE),
    ("SERIES_1", SERIES_1),
    ("SERIES_2", SERIES_2),
    ("DIV_BY_ZERO", DIV_BY_ZERO),
    ("PLATEAUS", PLATEAUS),
    ("CSV_METRIC_1", CSV_METRIC_1),
    ("CSV_METRIC_2", CSV_METRIC_2),
    ("TWO_POINTS", noisy(100, 2)),
    ("CONSTANT", [5.0] * 60),
    ("ONE_OUTLIER", ONE_OUTLIER),
    ("STEP_AT_LAST_POINT", STEP_AT_LAST_POINT),
    ("TWO_STEPS_IN_ONE_WINDOW", TWO_STEPS_IN_ONE_WINDOW),
    ("THREE_POINTS", THREE_POINTS),
    ("DOUBLING", DOUBLING),
    ("MERGE_NEIGHBORS", MERGE_NEIGHBORS),
    ("MERGE_NEIGHBORS_TAIL", MERGE_NEIGHBORS_TAIL),
    ("FIRST_PASS_AT_ONE", FIRST_PASS_AT_ONE),
    ("Q_TIE", Q_TIE),
    ("INTERVAL_TIE", INTERVAL_TIE),
    ("ODD_WINDOW", ODD_WINDOW),
    ("MAGNITUDE_TIE", MAGNITUDE_TIE),
    ("PAIRWISE_SUM", PAIRWISE_SUM),
    ("NEAR_CONSTANT", NEAR_CONSTANT),
    ("VERTICAL_SUM", VERTICAL_SUM),
]
CASES = (
    # The long series at the command line defaults, at Otava's own TigerBeetle
    # settings, and at the settings where the MongoDB series has steps.
    [(name, window, pvalue, magnitude)
     for name in ("TIGERBEETLE", "MONGODB_REAL")
     for (window, pvalue) in (DEFAULTS, (30, 0.01))
     for magnitude in (0.0, 0.05)]
    + [("MONGODB_REAL", window, pvalue, magnitude)
       for (window, pvalue) in ((10, 0.01), (20, 0.1), (50, 0.1))
       for magnitude in (0.0, 0.05)]
    + [("MONGODB_REAL", 50, 0.2, 0.0)]
    # Every setting of Otava's own TigerBeetle tests.
    + [("TIGERBEETLE", 30, pvalue, 0.05) for pvalue in (0.05, 0.1, 0.2)]
    + [("TIGERBEETLE", 30, pvalue, 0.0) for pvalue in (0.2, 0.1, 0.001, 0.0001, 0.00001)]
    + [
        ("SINGLE", 10, 0.0001, 0.0),
        ("SERIES_1", *DEFAULTS, 0.0),
        ("SERIES_1", *DEFAULTS, 0.2),
        ("SERIES_2", *DEFAULTS, 0.0),
        ("SERIES_2", *DEFAULTS, 0.2),
        ("DIV_BY_ZERO", *DEFAULTS, 0.0),
        ("PLATEAUS", 50, 0.05, 0.0),
        ("CSV_METRIC_1", *DEFAULTS, 0.0),
        ("CSV_METRIC_2", *DEFAULTS, 0.0),
        ("TWO_POINTS", *DEFAULTS, 0.0),
        # With no degrees of freedom the p value is 1, which the merge keeps only below a p value of 1.
        ("TWO_POINTS", 50, 1.0, 0.0),
        ("TWO_POINTS", 50, 2.0, 0.0),
        ("THREE_POINTS", *DEFAULTS, 0.0),
        # A magnitude of exactly 1 is kept only above a minimum magnitude of 1.
        ("DOUBLING", *DEFAULTS, 0.99),
        ("DOUBLING", *DEFAULTS, 1.0),
        ("CONSTANT", *DEFAULTS, 0.0),
        ("ONE_OUTLIER", *DEFAULTS, 0.0),
        ("ONE_OUTLIER", *DEFAULTS, 0.05),
        ("STEP_AT_LAST_POINT", *DEFAULTS, 0.0),
        ("TWO_STEPS_IN_ONE_WINDOW", *DEFAULTS, 0.0),
        ("MERGE_NEIGHBORS", 50, 0.001, 0.1),
        ("MERGE_NEIGHBORS_TAIL", 50, 0.001, 0.1),
        ("FIRST_PASS_AT_ONE", 6, 1.0, 0.0),
        ("Q_TIE", 100, 0.01, 0.2),
        ("INTERVAL_TIE", 12, 0.01, 0.03),
        ("ODD_WINDOW", 3, 0.001, 0.0),
        ("MAGNITUDE_TIE", 27, 0.001, 1.0),
        ("PAIRWISE_SUM", 10, 0.05, 0.5),
        ("NEAR_CONSTANT", 300, 0.001, 0.0),
        ("VERTICAL_SUM", 10, 1.0, 0.0),
        # Otava accepts a window of 2, where each side of a split in a window holds 1 point.
        ("SERIES_2", 2, 1.0, 0.05),
    ]
)


def analyze(values, window, pvalue, magnitude):
    series = Series(
        "series",
        branch=None,
        time=list(range(len(values))),
        metrics={"metric": Metric(1, 1.0)},
        data={"metric": list(values)},
        attributes={},
    )
    options = AnalysisOptions()
    options.window_len = window
    options.max_pvalue = pvalue
    options.min_magnitude = magnitude
    change_points = series.analyze(options).change_points
    # A metric with no change point has no entry.
    if "metric" not in change_points:
        return []
    return change_points.get_change_points_for_metric("metric")


def installed_commit():
    direct_url = json.loads(distribution("apache-otava").read_text("direct_url.json") or "{}")
    return direct_url.get("vcs_info", {}).get("commit_id")


def rust_f64(value):
    value = float(value)
    if math.isnan(value):
        return "f64::NAN"
    if math.isinf(value):
        return "f64::INFINITY" if value > 0 else "f64::NEG_INFINITY"
    return repr(value)


def rust_slice(name, values):
    lines = [f"pub const {name}: &[f64] = &["]
    for i in range(0, len(values), 6):
        lines.append("    " + " ".join(rust_f64(v) + "," for v in values[i:i + 6]))
    lines.append("];")
    return "\n".join(lines)


def main():
    assert installed_commit() == OTAVA, installed_commit()
    inputs = dict(INPUTS)
    out = ["//" + line.removeprefix("#") for line in HEADER.splitlines()]
    out += [
        "//",
        f"// Generated by `generate.py` with Apache Otava at commit {OTAVA}.",
        "// Do not edit it by hand.",
        "// It holds the inputs of `generate.py`, which names their sources, and the",
        "// changepoints that Otava's full analysis finds in them, translated to Rust",
        "// constants.",
        "",
        "pub struct Case {",
        "    pub series: &'static str,",
        "    pub values: &'static [f64],",
        "    pub window_len: usize,",
        "    pub max_pvalue: f64,",
        "    pub min_magnitude: f64,",
        "    pub changepoints: &'static [Expected],",
        "}",
        "",
        "pub struct Expected {",
        "    pub index: usize,",
        "    pub mean_before: f64,",
        "    pub mean_after: f64,",
        "    pub stddev_before: f64,",
        "    pub stddev_after: f64,",
        "    pub pvalue: f64,",
        "    pub tstatistic: f64,",
        "    pub degrees_of_freedom: usize,",
        "}",
        "",
    ]
    for name, values in INPUTS:
        out.append(rust_slice(name, values))
        out.append("")
    out.append("pub const CASES: &[Case] = &[")
    for name, window, pvalue, magnitude in CASES:
        changepoints = analyze(inputs[name], window, pvalue, magnitude)
        out += [
            "    Case {",
            f"        series: \"{name}\",",
            f"        values: {name},",
            f"        window_len: {window},",
            f"        max_pvalue: {rust_f64(pvalue)},",
            f"        min_magnitude: {rust_f64(magnitude)},",
            "        changepoints: &[",
        ]
        for c in changepoints:
            out += [
                "            Expected {",
                f"                index: {int(c.index)},",
                f"                mean_before: {rust_f64(c.stats.mean_before())},",
                f"                mean_after: {rust_f64(c.stats.mean_after())},",
                f"                stddev_before: {rust_f64(c.stats.stddev_before())},",
                f"                stddev_after: {rust_f64(c.stats.stddev_after())},",
                f"                pvalue: {rust_f64(c.stats.pvalue)},",
                f"                tstatistic: {rust_f64(c.stats.tstatistic)},",
                f"                degrees_of_freedom: {int(c.stats.degrees_of_freedom)},",
                "            },",
            ]
        out += ["        ],", "    },"]
    out.append("];")
    print("\n".join(out))


HEADER = open(__file__).read().split("# under the License.\n", 1)[0] + "# under the License."

if __name__ == "__main__":
    main()
