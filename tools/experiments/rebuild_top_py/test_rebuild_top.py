import json, tempfile, shutil, unittest
from pathlib import Path
import rebuild_top as rt

class TestGrouping(unittest.TestCase):
    def test_group_2x2(self):
        kids = [
            rt.RootChild('a', 6, 6, {}, '', Path('.'), Path('.'), 1, {}),
            rt.RootChild('b', 6, 7, {}, '', Path('.'), Path('.'), 1, {}),
            rt.RootChild('c', 5, 33, {}, '', Path('.'), Path('.'), 1, {}),
        ]
        g = rt.group_2x2(kids)
        self.assertEqual(len(g[(3,3)]), 2)
        self.assertEqual(len(g[(2,16)]), 1)

    def test_box_union(self):
        u = rt.box_union([
            [0,0,0, 1,0,0, 0,1,0, 0,0,1],
            [2,0,0, 1,0,0, 0,1,0, 0,0,1],
        ])
        self.assertAlmostEqual(u[0], 1.0)
        self.assertAlmostEqual(u[3], 2.0)

if __name__ == '__main__':
    unittest.main()
