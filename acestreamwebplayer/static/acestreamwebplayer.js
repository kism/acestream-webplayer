function getStreams() {
  const controller = new AbortController();

  const timeoutId = setTimeout(() => controller.abort(), 1000); // 1 second timeout:

  // GET the hello endpoint that the flask app has
  fetch("/api/v1/streams", {
    method: "GET",
    signal: controller.signal,
  })
    .then((response) => {
      // Check if the request was successful (status code 200)
      if (response.ok) {
        return response.json(); // We interoperate the response as json and pass it along...
      } else {
        document.getElementById("stream-status").innerHTML = `API FAILURE`; // Set message in element to indicate failure
        document.getElementById("stream-list").innerHTML = `${response.status}`; // Set message in element to message received from flask
        document.getElementById("stream-list").style.color = "#800000"; // Set message in element to message received from flask

        throw new Error(`Error fetching data. Status code: ${response.status}`);
      }
    })
    .then((data) => {
      let msg_str = ""; // Initialize an empty string to hold the message

      ele = document.getElementById("stream-list"); // The div for the stream list
      ele.innerHTML = ""; // Clear the inner HTML of the stream list element

      for (const site of data) {
        const ul = document.createElement("ul"); // Create a new unordered list element
        ul.className = "file-list"; // Set the class name for the unordered list
        ul.textContent = site.site_name; // Set the text content of the list to the site name

        for (const stream of site.stream_list) {
          const li = document.createElement("li"); // Create a new list item element
          const a = document.createElement("a"); // Create a new anchor element
          a.textContent = `${stream.title}`;
          a.onclick = () => loadStreamUrl(stream.ace_id, stream.title); // Set the onclick event to load the stream URL
          li.appendChild(a); // Append the anchor to the list item
          ul.appendChild(li); // Append the list item to the unordered list
        }
        ele.appendChild(ul); // Append the unordered list to the stream list element
      }

      document.getElementById("stream-status").style.display = "none"; // Hide the status element
      document.getElementById("stream-status").innerText = "No stream loaded"; // Set message in element to indicate success
    })
    .catch((error) => {
      clearTimeout(timeoutId); //Stop the timeout since we only care about the GET timing out
      if (error.name === "AbortError") {
        console.error("Fetch request timed out");
        document.getElementById("stream-status").innerHTML = `API FAILURE`; // Set message in element to indicate failure
        document.getElementById("stream-list").innerHTML = `Fetch Timeout`; // Set message in element to message received from flask
        document.getElementById("stream-list").style.color = "#800000"; // Set message in element to message received from flask
      } else {
        console.error(`Error: ${error.message}`);
      }
    });
}

function loadStream() {
  const video = document.getElementById("video");
  const videoSrc = `/hls/${window.location.hash.substring(1)}`;
  console.log(`Loading stream: ${videoSrc}`);
  if (Hls.isSupported()) {
    var hls = new Hls();
    hls.loadSource(videoSrc);
    hls.attachMedia(video);
  } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
    video.src = videoSrc; // For Safari
  } else {
    console.error("This browser does not support HLS playback.");
  }

  directURL = document.getElementById("direct-url");
  directURL.innerHTML = `${window.location.origin}${videoSrc}`;

  //start playing the video
  video.play().catch((error) => {
    console.error("Error playing video:", error);
    document.getElementById("stream-status").innerHTML = `Error playing video: ${error.message}`; // Set message in element to indicate error
  });
}

function loadStreamUrl(streamId, streamName) {
  window.location.hash = streamId; // Set the URL in the hash
  document.getElementById("stream-name").innerText = streamName;
  loadStream();
}

// Wrap DOM-dependent code in DOMContentLoaded event
document.addEventListener("DOMContentLoaded", function () {
  getStreams();
  setInterval(getStreams, 10 * 60 * 1000); // 10 minutes in milliseconds

  window.addEventListener("loadStream", loadStream);

  let streamId = window.location.hash.substring(1);
  console.log(`Stream ID from URL: ${streamId}`);
  if (streamId) {
    loadStreamUrl(streamId, streamId);
  }
});
