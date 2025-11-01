from PIL import Image
from io import BytesIO


def get_image_size(image:bytes):
    img = Image.open(BytesIO(image))
    return img.size
