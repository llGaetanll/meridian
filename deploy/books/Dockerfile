# The bookkeeper: a Python process that rewrites the Google Sheet once a
# day. docs/BOOKS.md is the whole of it.
#
# Built from the root of the repository:
#
#     docker build -f deploy/books/Dockerfile -t meridian-books:$(git rev-parse --short HEAD) .
#
# The context is filtered by `Dockerfile.dockerignore` beside this file rather
# than by the root `.dockerignore`, which is the server's.

FROM python:3.12-slim

WORKDIR /app

COPY books/requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt

COPY books/ ./

USER nobody

CMD ["python", "-u", "main.py"]
