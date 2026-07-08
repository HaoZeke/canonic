FROM python:3.12-alpine
WORKDIR /app
COPY server.py /app/server.py
EXPOSE 8080
USER nobody
CMD ["python3", "-u", "/app/server.py"]
