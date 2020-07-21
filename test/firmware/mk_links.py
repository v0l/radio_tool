#!/usr/bin/env python3
import sys
import hashlib
import os
import glob

BUF_SIZE = 65536

for file_in in glob.glob("*.bin"):
	sha = hashlib.sha256()

	with open(file_in, 'rb') as f:
		while True:
			data = f.read(BUF_SIZE)
			if not data:
				break
			sha.update(data)
		hashStr = sha.hexdigest()
		print("{0}: {1}".format(file_in, hashStr))
		with open("{0}.sha256".format(file_in.replace("(", "_").replace(")", "_").replace("-", "_")), 'w') as fsha:
			fsha.write(hashStr)
		out_link = "SHA256/{0}".format(hashStr)
		if not os.path.exists(out_link):
			os.symlink("../{0}".format(file_in), out_link)
