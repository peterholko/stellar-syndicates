"""Rebuild Second Wind with the shared trader reconstruction source."""
from pathlib import Path
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent.parent/'trader-fleet-v1'))
from build_models import build_one
if __name__=='__main__':
    build_one('second-wind')
