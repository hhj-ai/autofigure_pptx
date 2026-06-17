# Multimodal Fusion

The method encodes an image with a vision encoder and a caption with a text encoder. Both token streams are aligned in a shared fusion module, then passed to a lightweight task head for classification. The main contribution is the fusion module, which should be visually dominant.
